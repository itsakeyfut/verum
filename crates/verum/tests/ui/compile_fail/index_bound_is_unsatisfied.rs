//! `Index`'s own message, at a use site.
//!
//! `index_cannot_be_forged.rs` exercises the *seal* — it tries to implement `Index`
//! and is stopped before the trait's own wording is reached. So removing `Index`'s
//! entire `#[diagnostic::on_unimplemented]` block changed **zero bytes** across all
//! 27 `.stderr` files (measured). The message is good; nothing was holding it.
//!
//! This is the four-line fixture that holds it: a bound that simply is not satisfied,
//! so the trait's own text is what the reader sees.

pub struct NotAnIndex;

fn needs_index<I: verum::Index>() {}

fn main() {
    needs_index::<NotAnIndex>();
}
