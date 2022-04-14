#![doc = include_str!("../README.md")]

pub mod actors;
pub mod address;
pub mod auth;
mod crypto;
pub mod identity;
mod messaging;
mod net;
// pub mod trust_store;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
