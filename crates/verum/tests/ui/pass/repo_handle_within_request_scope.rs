//! The matching `pass` for `repo_handle_cannot_outlive_its_request`.
//!
//! Without this, an implementation where `Repo` could not be used *at all* — a
//! malformed declaration, an unsatisfiable bound — would still show a green
//! `compile_fail` suite. `'req` has to reject the escape **and** leave ordinary
//! use alone; the spike measured the same pair as E2/E4a against E3/E4b.

struct User;

/// Passing the handle around, returning it, and consuming it are all fine as long
/// as nothing demands it outlive `'req`.
fn takes_by_value<T>(_: T) {}

fn within_scope<'req>(r: verum::Repo<'req, User, (), ()>) {
    takes_by_value(r);
}

/// The handle may also be returned, so a service can receive one — the
/// parameterised handle is what `architecture-contract.md` says a service is
/// allowed to be given.
fn hands_it_on<'req>(r: verum::Repo<'req, User, (), ()>) -> verum::Repo<'req, User, (), ()> {
    r
}

fn main() {
    fn _uses<'req>(r: verum::Repo<'req, User, (), ()>) {
        within_scope(hands_it_on(r));
    }
}
