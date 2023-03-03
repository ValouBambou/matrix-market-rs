use matrix_market_rs::{MtxData, SymInfo};

#[test]
fn test_read_sparse_sym_small() {
    let output: MtxData<i32> = MtxData::from_file("small.mtx").unwrap();
    let expected_dims = [5, 5];
    let expected_values = vec![1, 2, 3, 4, 5, 6, 7];
    let expected_indices = vec![[0, 0], [0, 2], [1, 1], [1, 3], [2, 4], [3, 4], [4, 4]];
    use MtxData::*;
    match output {
        Sparse(dims, indices, values, sym) => {
            assert_eq!(dims, expected_dims, "Dimensions dont match");
            assert_eq!(values, expected_values, "Values dont match");
            assert_eq!(indices, expected_indices, "Values dont match");
            assert!(matches!(sym, SymInfo::Symmetric));
        }
        _dense => panic!("Expected Sparse not Dense"),
    }
}

#[test]
fn test_read_sparse_sym_big() {
    let output: MtxData<i32> = MtxData::from_file("big.mtx").unwrap();
    use MtxData::*;
    match output {
        Sparse(dims, _indices, _values, sym) => {
            assert_eq!(dims, [5120, 5120], "Dimensions dont match");
            assert!(matches!(sym, SymInfo::Symmetric));
        }
        _dense => panic!("Expected Sparse not Dense"),
    }
}

#[test]
fn test_read_dense() {
    let output: MtxData<i32> = MtxData::from_file("small_dense.mtx").unwrap();
    use MtxData::*;
    let expected_values = vec![1, 2, 3, 4, 5, 6];
    match output {
        Dense(dims, values, sym) => {
            assert_eq!(dims, [2, 3], "Dimensions dont match");
            assert_eq!(values, expected_values, "Values dont match");
            assert!(matches!(sym, SymInfo::General));
        }
        _sparse => panic!("Expected Dense not sparse"),
    }
}
