use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    str::FromStr,
};

use num_traits::Num;

#[derive(Debug)]
pub enum ErrorReadMtx {
    IoError(io::Error),
    EarlyEOF,
    EarlyBannerEnd,
    EarlyLineEnd,
    EarlySizesHeaderEOF,
    UnsupportedSym(String),
    UnsupportedNumType(String),
    UnsupportedLayout(String),
    InvalidNum(String),
}

impl From<io::Error> for ErrorReadMtx {
    fn from(value: io::Error) -> Self {
        ErrorReadMtx::IoError(value)
    }
}

pub enum SymInfo {
    General,
    Symmetric,
}

impl FromStr for SymInfo {
    type Err = ErrorReadMtx;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim_end() {
            "general" => Ok(SymInfo::General),
            "symmetric" => Ok(SymInfo::Symmetric),
            other => Err(ErrorReadMtx::UnsupportedSym(other.to_owned())),
        }
    }
}

pub enum MtxData<T: Num, const NDIM: usize = 2> {
    Dense([usize; NDIM], Vec<T>, SymInfo),
    Sparse([usize; NDIM], Vec<[usize; NDIM]>, Vec<T>, SymInfo),
}
const COMMENT: char = '%';

impl<T: Num, const NDIM: usize> MtxData<T, NDIM> {
    pub fn from_file(path: &str) -> Result<Self, ErrorReadMtx> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let (is_sparse, sym) = parse_banner(&mut reader, &mut line)?;
        skip_comments(&mut reader, &mut line)?;
        let (dims, nnz) = parse_sizes(&mut line)?;
        if is_sparse {
            let nnz = nnz.ok_or(ErrorReadMtx::EarlySizesHeaderEOF)?;
            let (indices, values) = parse_sparse_coo(&mut reader, &mut line, nnz)?;
            Ok(MtxData::Sparse(dims, indices, values, sym))
        } else {
            let capacity = dims.iter().product();
            let values = parse_dense_vec(&mut reader, &mut line, capacity)?;
            Ok(MtxData::Dense(dims, values, sym))
        }
    }
}

fn parse_sparse_coo<T: Num, const NDIM: usize>(
    reader: &mut BufReader<File>,
    buf: &mut String,
    nnz: usize,
) -> Result<(Vec<[usize; NDIM]>, Vec<T>), ErrorReadMtx> {
    let mut values: Vec<T> = Vec::with_capacity(nnz);
    let mut indices: Vec<[usize; NDIM]> = Vec::with_capacity(nnz);
    let mut n = reader.read_line(buf)?;
    while n > 0 {
        let (coords, val) = parse_coords_val(&buf)?;
        indices.push(coords);
        values.push(val);
        buf.clear();
        n = reader.read_line(buf)?;
    }
    Ok((indices, values))
}
fn parse_dense_vec<T: Num>(
    reader: &mut BufReader<File>,
    buf: &mut String,
    capacity: usize,
) -> Result<Vec<T>, ErrorReadMtx> {
    let mut v: Vec<T> = Vec::with_capacity(capacity);
    let mut n = 1;
    while n > 0 {
        n = reader.read_line(buf)?;
        match T::from_str_radix(buf.trim_end(), 10) {
            Ok(num) => {
                v.push(num);
            }
            Err(_) => {
                return Err(ErrorReadMtx::InvalidNum(buf.clone()));
            }
        }
        buf.clear();
    }
    Ok(v)
}
fn parse_coords_val<T: Num, const NDIM: usize>(
    line: &str,
) -> Result<([usize; NDIM], T), ErrorReadMtx> {
    let mut nnz: Option<T> = None;
    let mut dims = [0; NDIM];
    for (i, num) in line.trim_end().split_whitespace().enumerate() {
        if i == NDIM {
            let num =
                T::from_str_radix(num, 10).or(Err(ErrorReadMtx::InvalidNum(num.to_owned())))?;
            nnz = Some(num);
        } else {
            let num = usize::from_str(num).or(Err(ErrorReadMtx::InvalidNum(num.to_owned())))?;
            dims[i] = num - 1; // mtx is 1 based indexing while rust is 0
        }
    }
    if nnz.is_none() {
        Err(ErrorReadMtx::EarlyLineEnd)
    } else {
        Ok((dims, nnz.unwrap()))
    }
}

fn parse_sizes<const NDIM: usize>(
    buf: &mut String,
) -> Result<([usize; NDIM], Option<usize>), ErrorReadMtx> {
    let mut nnz: Option<usize> = None;
    let mut dims = [0; NDIM];
    for (i, num) in buf.trim_end().split_whitespace().enumerate() {
        let num = num
            .parse()
            .or(Err(ErrorReadMtx::InvalidNum(num.to_owned())))?;
        if i == NDIM {
            nnz = Some(num);
        } else {
            dims[i] = num;
        }
    }
    buf.clear();
    if dims.iter().any(|d| *d == 0) {
        Err(ErrorReadMtx::EarlySizesHeaderEOF)
    } else {
        Ok((dims, nnz))
    }
}

fn parse_banner(
    reader: &mut BufReader<File>,
    buf: &mut String,
) -> Result<(bool, SymInfo), ErrorReadMtx> {
    let n = reader.read_line(buf)?;
    if n == 0 {
        return Err(ErrorReadMtx::EarlyEOF);
    }

    // usually a banner look like this
    // %%MatrixMarket matrix coordinate integer symmetric
    // so we skip the 2 first fields and parse the next
    let mut banner = buf.split_whitespace().skip(2);
    let is_sparse = banner
        .next()
        .map(|c| c == "coordinate")
        .ok_or_else(|| ErrorReadMtx::EarlyBannerEnd)?;
    // so we skip the type since this already given with generic T
    let _type = banner.next().ok_or_else(|| ErrorReadMtx::EarlyBannerEnd);
    let sym = banner
        .next()
        .map(SymInfo::from_str)
        .ok_or_else(|| ErrorReadMtx::EarlyBannerEnd)??;
    buf.clear();

    Ok((is_sparse, sym))
}

fn skip_comments(reader: &mut BufReader<File>, buf: &mut String) -> Result<(), ErrorReadMtx> {
    let mut comment = true;
    while comment {
        buf.clear();
        let n = reader.read_line(buf)?;
        comment = buf.starts_with(COMMENT);
        if n == 0 {
            return Err(ErrorReadMtx::EarlyEOF);
        }
    }
    Ok(())
}
