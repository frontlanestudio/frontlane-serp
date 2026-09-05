use crate::core::types::{Envelope, ImageEnvelope, OutputFormat};

pub fn render_envelope(env: &Envelope, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(env).unwrap_or_default(),
        OutputFormat::Markdown => render_markdown(env),
        OutputFormat::Text => render_text(env),
        OutputFormat::Ndjson => render_ndjson(env),
    }
}

pub fn render_image_envelope(env: &ImageEnvelope, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(env).unwrap_or_default(),
        OutputFormat::Markdown => render_markdown_image(env),
        OutputFormat::Text => render_text_image(env),
        OutputFormat::Ndjson => render_ndjson_image(env),
    }
}

fn escape_markdown(s: &str) -> String {
    s.replace('[', "\\[")
        .replace(']', "\\]")
        .replace('*', "\\*")
        .replace('_', "\\_")
}

pub fn render_markdown(env: &Envelope) -> String {
    let mut b = String::new();
    let engines_str = env.query.engines_requested.join(", ");
    b.push_str(&format!("# Search results for \"{}\"\n\n", env.query.text));
    b.push_str(&format!(
        "**Query:** {} - **Engines:** {} - **Took:** {}ms\n\n",
        env.query.text, engines_str, env.meta.took_ms
    ));

    if !env.meta.engines_failed.is_empty() {
        b.push_str(&format!(
            "> Engines that failed: {}\n\n",
            env.meta.engines_failed.join(", ")
        ));
    }

    if !env.results.is_empty() {
        b.push_str("## Results\n\n");
    }

    for (i, r) in env.results.iter().enumerate() {
        b.push_str(&format!("### {}. {}\n\n", i + 1, escape_markdown(&r.title)));
        b.push_str(&format!("**{}** - {:?}\n\n", r.display_url, r.result_type));
        if !r.snippet.is_empty() {
            b.push_str(&format!("{}\n\n", r.snippet));
        }
        b.push_str(&format!("-> {}\n\n", r.url));
        if let Some(ref ext) = r.extracted {
            if let Some(ref content) = ext.content {
                if !content.is_empty() {
                    b.push_str("#### Extracted content\n\n");
                    b.push_str(content);
                    b.push_str("\n\n");
                }
            }
        }
    }

    b
}

pub fn render_markdown_image(env: &ImageEnvelope) -> String {
    let mut b = String::new();
    let engines_str = env.query.engines_requested.join(", ");
    b.push_str(&format!("# Image results for \"{}\"\n\n", env.query.text));
    b.push_str(&format!(
        "**Query:** {} - **Engines:** {} - **Took:** {}ms\n\n",
        env.query.text, engines_str, env.meta.took_ms
    ));

    for (i, r) in env.results.iter().enumerate() {
        b.push_str(&format!("## {}. {}\n\n", i + 1, escape_markdown(&r.title)));
        b.push_str(&format!("**Source:** {}\n\n", r.source.domain));
        b.push_str(&format!("-> Image: {}\n", r.image.url));
        b.push_str(&format!("-> Page: {}\n\n", r.source.page_url));
    }

    b
}

pub fn render_text(env: &Envelope) -> String {
    let mut b = String::new();
    b.push_str(&format!("Search: {}\n", env.query.text));
    let engines_str = env.query.engines_requested.join(", ");
    if !engines_str.is_empty() {
        b.push_str(&format!("Engines: {}\n", engines_str));
    }
    if !env.meta.engines_failed.is_empty() {
        b.push_str(&format!("Failed: {}\n", env.meta.engines_failed.join(", ")));
    }
    b.push('\n');

    if !env.results.is_empty() {
        b.push_str("Results\n\n");
    }

    for (i, r) in env.results.iter().enumerate() {
        b.push_str(&format!("[{}] {} ({})\n", i + 1, r.title, r.domain));
        if !r.snippet.is_empty() {
            b.push_str(&format!("{}\n", r.snippet));
        }
        b.push_str(&format!("URL: {}\n\n", r.url));
        if let Some(ref ext) = r.extracted {
            if let Some(ref content) = ext.content {
                if !content.is_empty() {
                    b.push_str("Extracted content:\n");
                    b.push_str(content);
                    b.push_str("\n\n");
                }
            }
        }
    }

    b
}

pub fn render_text_image(env: &ImageEnvelope) -> String {
    let mut b = String::new();
    b.push_str(&format!("Image search: {}\n\n", env.query.text));

    for (i, r) in env.results.iter().enumerate() {
        b.push_str(&format!("[{}] {} ({})\n", i + 1, r.title, r.source.domain));
        b.push_str(&format!("Image: {}\n", r.image.url));
        b.push_str(&format!("Page: {}\n\n", r.source.page_url));
    }

    b
}

pub fn render_ndjson(env: &Envelope) -> String {
    let mut b = String::new();
    for r in &env.results {
        let line = serde_json::json!({ "type": "result", "data": r });
        b.push_str(&line.to_string());
        b.push('\n');
    }
    for f in &env.serp_features {
        let line = serde_json::json!({ "type": "feature", "data": f });
        b.push_str(&line.to_string());
        b.push('\n');
    }
    b
}

pub fn render_ndjson_image(env: &ImageEnvelope) -> String {
    let mut b = String::new();
    for r in &env.results {
        let line = serde_json::json!({ "type": "result", "data": r });
        b.push_str(&line.to_string());
        b.push('\n');
    }
    b
}
