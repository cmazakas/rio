#![allow(non_camel_case_types)]

pub mod libc;
pub mod liburing;
pub mod probe;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
