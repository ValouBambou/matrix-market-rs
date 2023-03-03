use std::{
    error::Error,
    fmt::Display,
    fs::File,
    io::{self, BufRead, BufReader},
    num::ParseIntError,
    str::FromStr,
};

use num_traits::Num;

/// List all the possibles errors that could occurs.
#[derive(Debug)]
pub enum MtxError {
    IoError(io::Error),
    EarlyEOF,
    EarlyBannerEnd,
    EarlyLineEnd,
    EarlySizesHeaderEnd,
    UnsupportedSym(String),
    UnsupportedNumType(String),
    UnsupportedLayout(String),
    InvalidNum(String),
    InvalidCoordinate(ParseIntError),
}

impl Display for MtxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use MtxError::*;
        let msg = match self {
            IoError(_) => "IO error occurs when manipulate mtx file",
            _ => "Invalid mtx text format",
        };
        write!(f, "{msg}")
    }
}

impl From<io::Error> for MtxError {
    fn from(value: io::Error) -> Self {
        MtxError::IoError(value)
    }
}

impl From<ParseIntError> for MtxError {
    fn from(value: ParseIntError) -> Self {
        MtxError::InvalidCoordinate(value)
    }
}

impl Error for MtxError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        use MtxError::*;
        match self {
            IoError(e) => Some(e),
            InvalidCoordinate(e) => Some(e),
            _ => None,
        }
    }
}

/// Symmetry information in the matrix market banner.
/// Currently we dont support all of the info available in the format.
/// Because we dont handle complex numbers.
/// Feel free to contribute and add the missing support for those numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymInfo {
    General,
    Symmetric,
}

impl FromStr for SymInfo {
    type Err = MtxError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim_end() {
            "general" => Ok(SymInfo::General),
            "symmetric" => Ok(SymInfo::Symmetric),
            other => Err(MtxError::UnsupportedSym(other.to_owned())),
        }
    }
}

/// The main enum of this crate, corresponding to the 2 kind of usage of mtx files.
/// Both contains a first line with dimensions.
/// Dense is a list of numbers.
/// Sparse is a list of coordinates and values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MtxData<T: Num, const NDIM: usize = 2> {
    Dense([usize; NDIM], Vec<T>, SymInfo),
    Sparse([usize; NDIM], Vec<[usize; NDIM]>, Vec<T>, SymInfo),
}

impl<T: Num, const NDIM: usize> MtxData<T, NDIM> {
    /// Build a `MtxData` from a matrix market (usually .mtx) file.
    ///
    /// # Example
    /// ```rust
    /// use matrix_market_rs::{MtxData, SymInfo, MtxError};
    /// use std::fs::File;
    /// use std::io::Write;
    ///
    /// fn main() -> Result<(), MtxError> {
    ///     let mtx_content = r#"
    ///     %%MatrixMarket matrix coordinate integer symmetric
    ///     2 2 2
    ///     1 1 3
    ///     2 2 4
    ///     "#;
    ///    
    ///     let mut f = File::create("sparse2x2.mtx")?;
    ///     f.write_all(mtx_content.trim().as_bytes());
    ///     let shape = [2,2];
    ///     let indices = vec![[0,0], [1,1]];
    ///     let nonzeros = vec![3,4];
    ///     let sym = SymInfo::Symmetric;
    ///    
    ///     let sparse:MtxData<i32> = MtxData::from_file("sparse2x2.mtx")?;
    ///     assert_eq!(sparse, MtxData::Sparse(shape, indices, nonzeros, sym));
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// It could fail for many reasons but for example:
    /// - File doesn't match the matrix market format.
    /// - an IO error (file not found etc.)
    pub fn from_file(path: &str) -> Result<Self, MtxError> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let (is_sparse, sym) = parse_banner(&mut reader, &mut line)?;
        skip_comments(&mut reader, &mut line)?;
        let (dims, nnz) = parse_sizes(&mut line)?;
        if is_sparse {
            let nnz = nnz.ok_or(MtxError::EarlySizesHeaderEnd)?;
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
) -> Result<(Vec<[usize; NDIM]>, Vec<T>), MtxError> {
    let mut values: Vec<T> = Vec::with_capacity(nnz);
    let mut indices: Vec<[usize; NDIM]> = Vec::with_capacity(nnz);
    for _ in 0..nnz {
        let n = reader.read_line(buf)?;
        if n == 0 {
            return Err(MtxError::EarlyEOF);
        }
        let (coords, val) = parse_coords_val(buf)?;
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
) -> Result<Vec<T>, MtxError> {
    let mut v: Vec<T> = Vec::with_capacity(capacity);
    for _ in 0..capacity {
        let n = reader.read_line(buf)?;
        if n == 0 {
            return Err(MtxError::EarlyEOF);
        }
        match T::from_str_radix(buf.trim_end(), 10) {
            Ok(num) => {
                v.push(num);
            }
            Err(_) => {
                return Err(MtxError::InvalidNum(buf.clone()));
            }
        }
        buf.clear();
    }
    Ok(v)
}
fn parse_coords_val<T: Num, const NDIM: usize>(line: &str) -> Result<([usize; NDIM], T), MtxError> {
    let mut value: Option<T> = None;
    let mut dims = [0; NDIM];
    for (i, num) in line.split_whitespace().enumerate() {
        if i == NDIM {
            let num = T::from_str_radix(num, 10).or(Err(MtxError::InvalidNum(num.to_owned())))?;
            value = Some(num);
        } else {
            let num = usize::from_str(num)?;
            dims[i] = num - 1; // mtx is 1 based indexing while rust is 0
        }
    }
    if let Some(val) = value {
        Ok((dims, val))
    } else {
        Err(MtxError::EarlyLineEnd)
    }
}

fn parse_sizes<const NDIM: usize>(
    buf: &mut String,
) -> Result<([usize; NDIM], Option<usize>), MtxError> {
    let mut nnz: Option<usize> = None;
    let mut dims = [0; NDIM];
    for (i, num) in buf.split_whitespace().enumerate() {
        let num = usize::from_str(num)?;
        if i == NDIM {
            nnz = Some(num);
        } else {
            dims[i] = num;
        }
    }
    println!("buf = {buf}, dims = {dims:?}");
    buf.clear();
    if dims.iter().any(|d| *d == 0) {
        Err(MtxError::EarlySizesHeaderEnd)
    } else {
        Ok((dims, nnz))
    }
}

fn parse_banner(
    reader: &mut BufReader<File>,
    buf: &mut String,
) -> Result<(bool, SymInfo), MtxError> {
    let n = reader.read_line(buf)?;
    if n == 0 {
        return Err(MtxError::EarlyEOF);
    }

    // usually a banner look like this
    // %%MatrixMarket matrix coordinate integer symmetric
    // so we skip the 2 first fields and parse the next
    println!("banner = {buf}");
    let mut banner = buf.split_whitespace().skip(2);
    let is_sparse = banner
        .next()
        .map(|c| c == "coordinate")
        .ok_or_else(|| MtxError::EarlyBannerEnd)?;
    // so we skip the type since this already given with generic T
    let _type = banner.next().ok_or_else(|| MtxError::EarlyBannerEnd);
    let sym = banner
        .next()
        .map(SymInfo::from_str)
        .ok_or_else(|| MtxError::EarlyBannerEnd)??;
    buf.clear();

    Ok((is_sparse, sym))
}

const COMMENT: char = '%';
fn skip_comments(reader: &mut BufReader<File>, buf: &mut String) -> Result<(), MtxError> {
    let mut comment = true;
    while comment {
        buf.clear();
        let n = reader.read_line(buf)?;
        comment = buf.starts_with(COMMENT);
        if n == 0 {
            return Err(MtxError::EarlyEOF);
        }
    }
    Ok(())
}
