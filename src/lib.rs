use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    str::FromStr,
};

use num_traits::Num;

/// List all the possibles errors that could occurs.
#[derive(Debug)]
pub enum ErrorReadMtx {
    IoError(io::Error),
    EarlyEOF,
    EarlyBannerEnd,
    EarlyLineEnd,
    EarlySizesHeaderEnd,
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

/// Symmetry information in the matrix market banner.
/// Currently we dont support all of the info available in the format.
/// Because we dont handle complex numbers.
/// Feel free to contribute and add the missing support for those numbers.
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

/// The main enum of this crate, corresponding to the 2 kind of usage of mtx files.
/// Both contains a first line with dimensions.
/// Dense is a list of numbers.
/// Sparse is a list of coordinates and values.
pub enum MtxData<T: Num, const NDIM: usize = 2> {
    Dense([usize; NDIM], Vec<T>, SymInfo),
    Sparse([usize; NDIM], Vec<[usize; NDIM]>, Vec<T>, SymInfo),
}

impl<T: Num, const NDIM: usize> MtxData<T, NDIM> {
    /// Build a `MtxData` from a matrix market (usually .mtx) file.
    /// It could fail for many reasons but for example:
    /// - File doesn't match the matrix market format.
    /// - an IO error (file not found etc.)
    pub fn from_file(path: &str) -> Result<Self, ErrorReadMtx> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let (is_sparse, sym) = parse_banner(&mut reader, &mut line)?;
        skip_comments(&mut reader, &mut line)?;
        let (dims, nnz) = parse_sizes(&mut line)?;
        if is_sparse {
            let nnz = nnz.ok_or(ErrorReadMtx::EarlySizesHeaderEnd)?;
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
    for _ in 0..nnz {
        let n = reader.read_line(buf)?;
        if n == 0 {
            return Err(ErrorReadMtx::EarlyEOF);
        }
        let (coords, val) = parse_coords_val(&buf)?;
        indices.push(coords);
        values.push(val);
        buf.clear();
    }
    Ok((indices, values))
}
fn parse_dense_vec<T: Num>(
    reader: &mut BufReader<File>,
    buf: &mut String,
    capacity: usize,
) -> Result<Vec<T>, ErrorReadMtx> {
    let mut v: Vec<T> = Vec::with_capacity(capacity);
    for _ in 0..capacity {
        let n = reader.read_line(buf)?;
        if n == 0 {
            return Err(ErrorReadMtx::EarlyEOF);
        }
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
    println!("buf = {buf}, dims = {dims:?}");
    buf.clear();
    if dims.iter().any(|d| *d == 0) {
        Err(ErrorReadMtx::EarlySizesHeaderEnd)
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

const COMMENT: char = '%';
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
