/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 */

//! Benchmark utilities for testing mirror speeds.

use super::mirror::MirrorPool;
use anyhow::Result;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Result of a mirror benchmark
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub url: String,
    pub latency_ms: u64,
    pub throughput_mbps: f64,
    pub success: bool,
    pub error: Option<String>,
}

impl BenchmarkResult {
    pub fn success(url: String, latency_ms: u64, throughput_mbps: f64) -> Self {
        Self {
            url,
            latency_ms,
            throughput_mbps,
            success: true,
            error: None,
        }
    }

    pub fn failure(url: String, error: String) -> Self {
        Self {
            url,
            latency_ms: 0,
            throughput_mbps: 0.0,
            success: false,
            error: Some(error),
        }
    }
}

/// Run benchmark on a set of mirrors with multiple iterations for scientific accuracy
pub async fn run_benchmark(mirrors: Vec<String>, test_size_kb: u64) -> Result<Vec<BenchmarkResult>> {
    const ITERATIONS: usize = 3;
    println!("{}", style(":: Running benchmark...").bold().cyan());
    println!("   {} mirrors | {} KB test size | {} iterations\n", mirrors.len(), test_size_kb, ITERATIONS);

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(5))
        .build()?;

    let pb = ProgressBar::new(mirrors.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );

    let mut results = Vec::new();

    for url in mirrors {
        pb.set_message(format!("Testing: {}", truncate_url(&url, 40)));
        
        let mut iteration_results = Vec::new();
        for _ in 0..ITERATIONS {
            let result = benchmark_mirror(&client, &url, test_size_kb).await;
            if result.success {
                iteration_results.push(result);
            }
        }

        if iteration_results.is_empty() {
             results.push(BenchmarkResult::failure(url.to_string(), "All iterations failed".to_string()));
        } else {
            // Calculate median for robustness
            iteration_results.sort_by(|a, b| a.throughput_mbps.partial_cmp(&b.throughput_mbps).unwrap());
            let median = iteration_results[iteration_results.len() / 2].clone();
            results.push(median);
        }
        
        pb.inc(1);
    }

    pb.finish_and_clear();

    // Sort by throughput (best first)
    results.sort_by(|a, b| {
        b.throughput_mbps
            .partial_cmp(&a.throughput_mbps)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Print results
    print_benchmark_results(&results);

    Ok(results)
}

async fn benchmark_mirror(client: &Client, base_url: &str, test_size_kb: u64) -> BenchmarkResult {
    // Try to download the database file to test speed
    let test_urls = vec![
        format!("{}/core.db", base_url),
        format!("{}/extra.db", base_url),
    ];

    for test_url in test_urls {
        let start = Instant::now();
        
        match client.get(&test_url).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    continue;
                }

                let latency = start.elapsed().as_millis() as u64;

                // Download content to measure throughput
                match response.bytes().await {
                    Ok(bytes) => {
                        let elapsed = start.elapsed();
                        let bytes_downloaded = bytes.len() as f64;
                        let seconds = elapsed.as_secs_f64();
                        
                        if seconds > 0.0 {
                            let throughput_mbps = (bytes_downloaded / 1024.0 / 1024.0) / seconds * 8.0;
                            return BenchmarkResult::success(base_url.to_string(), latency, throughput_mbps);
                        }
                    }
                    Err(e) => {
                        return BenchmarkResult::failure(base_url.to_string(), e.to_string());
                    }
                }
            }
            Err(e) => {
                return BenchmarkResult::failure(base_url.to_string(), e.to_string());
            }
        }
    }

    BenchmarkResult::failure(base_url.to_string(), "No test files accessible".to_string())
}

fn print_benchmark_results(results: &[BenchmarkResult]) {
    println!("\n{}", style("Mirror Benchmark Results").bold().cyan());
    println!("{}", style("─".repeat(80)).dim());
    println!(
        "{:>5} {:>10} {:>12}  {}",
        style("Rank").bold(),
        style("Latency").bold(),
        style("Speed").bold(),
        style("Mirror").bold()
    );
    println!("{}", style("─".repeat(80)).dim());

    for (i, result) in results.iter().enumerate() {
        let rank = i + 1;
        
        if result.success {
            let latency = format!("{}ms", result.latency_ms);
            let speed = format!("{:.2} Mbps", result.throughput_mbps);
            
            let speed_style = if result.throughput_mbps > 100.0 {
                style(speed).green().bold()
            } else if result.throughput_mbps > 50.0 {
                style(speed).green()
            } else if result.throughput_mbps > 10.0 {
                style(speed).yellow()
            } else {
                style(speed).red()
            };

            println!(
                "{:>5} {:>10} {:>12}  {}",
                style(format!("#{}", rank)).cyan().bold(),
                latency,
                speed_style,
                truncate_url(&result.url, 45)
            );
        } else {
            println!(
                "{:>5} {:>10} {:>12}  {} {}",
                style(format!("#{}", rank)).dim(),
                style("---").dim(),
                style("FAILED").red(),
                truncate_url(&result.url, 35),
                style(format!("({})", result.error.as_deref().unwrap_or("unknown"))).dim()
            );
        }
    }

    println!("{}", style("─".repeat(80)).dim());

    // Summary
    let successful: Vec<_> = results.iter().filter(|r| r.success).collect();
    if !successful.is_empty() {
        let avg_speed: f64 = successful.iter().map(|r| r.throughput_mbps).sum::<f64>() / successful.len() as f64;
        let max_speed = successful.iter().map(|r| r.throughput_mbps).fold(0.0f64, f64::max);
        
        println!("\n{}", style("Summary").bold());
        println!("  Mirrors tested: {}", results.len());
        println!("  Successful:     {} ({}%)", 
            successful.len(), 
            (successful.len() * 100) / results.len()
        );
        println!("  Average speed:  {:.2} Mbps", avg_speed);
        println!("  Best speed:     {:.2} Mbps", max_speed);
        
        if let Some(best) = successful.first() {
            println!("  Fastest mirror: {}", style(&best.url).green());
        }
    }
}

fn truncate_url(url: &str, max_len: usize) -> String {
    if url.len() <= max_len {
        url.to_string()
    } else {
        format!("{}...", &url[..max_len - 3])
    }
}
