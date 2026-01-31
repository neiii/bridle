use crate::error::Error;

pub fn run_tui() -> Result<(), Error> {
    crate::tui::run()
}
