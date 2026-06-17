use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};

fn main() {
    let argon2 = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);
    let password = b"admin123!";
    let hash = argon2.hash_password(password, &salt).unwrap();
    println!("{}", hash);
}
