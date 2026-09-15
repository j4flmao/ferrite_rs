use crate::templates;

/// `fr templates --list` (default) or `fr templates --gallery` (pretty table).
pub fn run(gallery: bool) -> anyhow::Result<()> {
    let rows = templates::gallery();
    if gallery {
        let slug_w = rows.iter().map(|r| r.0.len()).max().unwrap_or(4);
        let title_w = rows.iter().map(|r| r.1.len()).max().unwrap_or(10);
        let n_w = rows
            .iter()
            .map(|r| r.3.ilog10() as usize + 1)
            .max()
            .unwrap_or(1);
        println!(
            " {:<slug_w$}  {:<title_w$}  {:>n_w$} files  Description",
            "TEMPLATE",
            "TITLE",
            "FILES",
            slug_w = slug_w.max(8),
            title_w = title_w.max(10),
            n_w = n_w.max(5)
        );
        println!(
            " {}  {}  {}  {}",
            "-".repeat(slug_w.max(8)),
            "-".repeat(title_w.max(10)),
            "-".repeat(n_w.max(5)),
            "-".repeat(60)
        );
        for (slug, title, desc, files) in &rows {
            println!(
                " \x1b[32m{:<slug_w$}\x1b[0m  {:<title_w$}  \x1b[33m{:>n_w$}\x1b[0m        {desc}",
                slug,
                title,
                files,
                slug_w = slug_w.max(8),
                title_w = title_w.max(10),
                n_w = n_w.max(5)
            );
        }
        println!();
        println!("  Usage: fr new <name> --template <template>");
    } else {
        println!("Available templates for `fr new --template <slug>`:\n");
        for (slug, title, desc, files) in &rows {
            println!("  · \x1b[32m{slug}\x1b[0m — {title} [{files} files]\n      {desc}");
        }
    }
    Ok(())
}
