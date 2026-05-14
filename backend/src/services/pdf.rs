use anyhow::Result;
use reqwest::Client;

pub struct PdfService {
    client: Client,
}

impl PdfService {
    pub fn new() -> Self {
        Self { client: Client::new() }
    }

    /// Download a PDF from a URL and extract its text content
    pub async fn download_and_extract(&self, url: &str) -> Result<String> {
        tracing::info!("Downloading PDF from: {}", url);

        let resp = self.client
            .get(url)
            .header("User-Agent", "LivePaper/1.0 (research tool)")
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("PDF download failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("PDF download HTTP {}", resp.status()));
        }

        let bytes = resp.bytes().await?;
        tracing::info!("Downloaded {} bytes", bytes.len());

        let text = pdf_extract::extract_text_from_mem(&bytes)
                    .map_err(|e| anyhow::anyhow!("PDF text extraction failed: {}", e))?;

        // Clean up extracted text
        let cleaned = clean_pdf_text(&text);
        tracing::info!("Extracted {} chars from PDF", cleaned.len());
        Ok(cleaned)
    }

    /// Chunk text into overlapping segments for embedding
    pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut chunks = Vec::new();

        let mut i = 0;
        while i < words.len() {
            let end = (i + chunk_size).min(words.len());
            let chunk = words[i..end].join(" ");
            if !chunk.trim().is_empty() {
                chunks.push(chunk);
            }
            if end == words.len() {
                break;
            }
            i += chunk_size - overlap;
        }
        chunks
    }
}

fn clean_pdf_text(text: &str) -> String {
    text.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| c.is_ascii() || c.is_alphabetic() || *c == ' ' || *c == '.' || *c == ',' || *c == '\n')
        .collect()
}
