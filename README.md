# matrix-market-rs

A simple reader/parser for Matrix Market (.mtx) files to represent sparse or dense matrix in text format.

## How to use it ?

Add this to the dependencies in your Cargo.toml.

```toml
matrix-market-rs = "0.1"
```

And then use it in your program.

```rust
use matrix_market_rs::{MtxData, SymInfo, MtxError};
use std::fs::File;
use std::io::Write;

fn main() -> Result<(), MtxError> {
    let mtx_content = r#"
    %%MatrixMarket matrix coordinate integer symmetric
    2 2 2
    1 1 3
    2 2 4
    "#;

    let mut f = File::create("sparse2x2.mtx")?;
    f.write_all(mtx_content.trim().as_bytes());
    let shape = [2,2];
    let indices = vec![[0,0], [1,1]];
    let nonzeros = vec![3,4];
    let sym = SymInfo::Symmetric;

    let sparse:MtxData<i32> = MtxData::from_file("sparse2x2.mtx")?;
    assert_eq!(sparse, MtxData::Sparse(shape, indices, nonzeros, sym));
    Ok(())
}
```
