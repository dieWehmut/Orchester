use std::io::{self, Write};
pub fn edit_model(default_name: &str, default_model: &str) -> io::Result<Option<(String, String)>> {
    fn field(label: &str, default: &str) -> io::Result<Option<String>> {
        print!("{label} [{default}] (enter keeps, /cancel cancels): ");
        io::stdout().flush()?;
        let mut text = String::new();
        if io::stdin().read_line(&mut text)? == 0 {
            return Ok(None);
        }
        let text = text.trim();
        if text == "/cancel" {
            return Ok(None);
        }
        Ok(Some(if text.is_empty() {
            default.to_owned()
        } else {
            text.to_owned()
        }))
    }
    let Some(name) = field("Model alias", default_name)? else {
        return Ok(None);
    };
    if name.is_empty() {
        return Ok(None);
    }
    let Some(model) = field("Model ID", default_model)? else {
        return Ok(None);
    };
    Ok(Some((name, model)))
}
