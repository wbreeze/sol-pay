#[cfg(test)]
mod test_initialize;

#[cfg(test)]
mod tests {

  fn divide(num: usize, den: usize) -> f32 {
    (num as f32) / (den as f32)
  }

  #[test]
  fn test_divide() {
    let r = divide(4,2);
    assert_eq!(r, 2.0);
  }
}
