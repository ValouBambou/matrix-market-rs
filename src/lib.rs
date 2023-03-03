use num_traits::Float;
use std::str::FromStr;

const ERR_EMPTY: &str = "File is empty (or contains only comments)";
const ERR_PARSING_INT: &str = "Expected int";
const ERR_PARSING_FLOAT: &str = "Expected float";

fn shape_and_lines_from_content(content: &String) -> (impl Iterator<Item = &str>, Vec<usize>) {
    let mut lines = content
        .lines()
        .map(|line| line.trim())
        .filter(|line| (!line.starts_with('%')) && (!line.is_empty()));
    // first line contains shape
    let shape: Vec<usize> = lines
        .next()
        .expect(ERR_EMPTY)
        .split_whitespace()
        .map(|len| len.parse().expect(ERR_PARSING_INT))
        .collect();
    (lines, shape)
}

fn parse_float<F: Float + FromStr>(line: &str) -> F {
    if let Ok(f) = line.parse::<F>() {
        f
    } else {
        panic!("{ERR_PARSING_FLOAT}, not {line}");
    }
}
fn parse_dense<F: Float + FromStr>(content: String) -> (Vec<usize>, Vec<F>) {
    let (lines, shape) = shape_and_lines_from_content(&content);
    // next lines contains values in a column major order for dense matrix
    let values: Vec<F> = lines.map(parse_float).collect();
    (shape, values)
}

pub fn dense_from_file<F: Float + FromStr>(path: &str) -> (Vec<usize>, Vec<F>) {
    let content = std::fs::read_to_string(path).unwrap();
    parse_dense(content)
}

fn parse_sparse<F: Float + FromStr>(content: String) -> (Vec<usize>, Vec<Vec<usize>>, Vec<F>) {
    let (lines, mut shape) = shape_and_lines_from_content(&content);
    // but last number is the number of non zeros
    let len_nonzeros = shape.pop().unwrap();
    let mut nonzeros: Vec<F> = Vec::with_capacity(len_nonzeros);
    let mut indices: Vec<Vec<usize>> = Vec::with_capacity(len_nonzeros);
    for line in lines {
        let mut it: Vec<&str> = line.split_whitespace().collect();
        let value = parse_float(it.pop().unwrap());
        nonzeros.push(value);
        let index: Vec<usize> = it
            .into_iter()
            .map(|x| x.parse::<usize>().expect(ERR_PARSING_INT) - 1)
            .collect();
        indices.push(index)
    }
    (shape, indices, nonzeros)
}

pub fn sparse_from_file<F: Float + FromStr>(path: &str) -> (Vec<usize>, Vec<Vec<usize>>, Vec<F>) {
    let content = std::fs::read_to_string(path).unwrap();
    parse_sparse(content)
}

#[cfg(test)]
mod tests_mm_parse {
    use super::*;
    #[test]
    #[should_panic(expected = "Expected int")]
    fn test_fail_parse_shape() {
        let content = "some garbage content".to_owned();
        parse_dense::<f32>(content);
    }

    #[test]
    #[should_panic(expected = "File is empty (or contains only comments)")]
    fn test_fail_parse_empty() {
        let content = "% some comment\n\n\t\n% another comment".to_owned();
        parse_dense::<f32>(content);
    }

    #[test]
    #[should_panic(expected = "Expected float")]
    fn test_fail_parse_dense() {
        let content = "% some comment\n10 10\n0.0\ngarbage".to_owned();
        parse_dense::<f32>(content);
    }

    #[test]
    #[should_panic(expected = "Expected int")]
    fn test_fail_parse_sparse() {
        let content = "% some comment\n10 10 2\n1 1 0.42\ngarbage 2 0.7".to_owned();
        parse_sparse::<f32>(content);
    }

    #[test]
    #[should_panic(expected = "Expected float")]
    fn test_fail_parse_sparse2() {
        let content = "% some comment\n10 10 2\n1 1 0.42\n6 2 garbage".to_owned();
        parse_sparse::<f32>(content);
    }
    #[test]
    fn test_parse_sparse() {
        let content = "10 11 2\n1 1    0.42\n6 2 0.7".to_owned();
        let (shape, indices, nonzeros) = parse_sparse::<f32>(content);
        assert_eq!(shape, vec![10, 11], "shape differ from expected");
        assert_eq!(
            indices,
            vec![vec![0, 0], vec![5, 1]],
            "indices differ from expected"
        );
        assert_eq!(
            nonzeros,
            vec![0.42, 0.7],
            "nonzeros values differ from expected"
        );
    }

    #[test]
    fn test_parse_dense() {
        let content = "2    3\n0.1\n0.2\n0.3\n0.4\n0.5\n0.6".to_owned();
        let (shape, values) = parse_dense::<f32>(content);
        assert_eq!(shape, vec![2, 3], "shape differ from expected");
        assert_eq!(
            values,
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
            "values differ from expected"
        );
    }
}
