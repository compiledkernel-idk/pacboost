/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use alpm::{Alpm, SigLevel, LogLevel};
use anyhow::{Result, anyhow};
use std::path::Path;
use indicatif::MultiProgress;

pub struct AlpmManager {
    pub handle: Alpm,
    pub dbpath: String,
}

impl AlpmManager {
    pub fn new() -> Result<Self> {
        let root = "/";
        let dbpath = "/var/lib/pacman";
        
        let mut handle = Alpm::new(root, dbpath).map_err(|e| anyhow!("failed to initialize libalpm: {}", e))?;

        // Set Cache Directory
        handle.add_cachedir("/var/cache/pacman/pkg/").map_err(|e| anyhow!("failed to set cachedir: {}", e))?;

        // Logging Callback
        handle.set_log_cb((), |level, msg, _| {
             match level {
                 LogLevel::ERROR => eprint!("alpm error: {}", msg),
                 LogLevel::WARNING => eprint!("alpm warning: {}", msg),
                 _ => {}, 
             }
        });

        // Question Callback
        handle.set_question_cb((), |any_question, _| {
             use alpm::Question;
             match any_question.question() {
                 Question::Replace(q) => {
                     eprintln!(":: replacing {} with {}...", q.oldpkg().name(), q.newpkg().name());
                     q.set_replace(true);
                 },
                 _ => {},
             }
        });
        
        // Parse /etc/pacman.conf to get all configured repositories
        let repos = Self::parse_pacman_conf()?;
        
        for (repo_name, servers) in repos {
            let db = handle.register_syncdb_mut(repo_name.as_str(), SigLevel::DATABASE_OPTIONAL)?;
            for server in servers {
                db.add_server(server.as_str())?;
            }
        }

        Ok(Self { 
            handle,
            dbpath: dbpath.to_string(),
        })
    }
    
    /// Parse /etc/pacman.conf to extract repository names and servers
    fn parse_pacman_conf() -> Result<Vec<(String, Vec<String>)>> {
        use std::fs;
        
        let conf_content = fs::read_to_string("/etc/pacman.conf")
            .map_err(|e| anyhow!("failed to read /etc/pacman.conf: {}", e))?;
        
        let mut repos = Vec::new();
        let mut current_repo: Option<String> = None;
        let mut current_servers = Vec::new();
        
        for line in conf_content.lines() {
            let line = line.trim();
            
            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            // Check for repository section [repo]
            if line.starts_with('[') && line.ends_with(']') {
                // Save previous repo if exists
                if let Some(repo) = current_repo.take() {
                    if !current_servers.is_empty() {
                        repos.push((repo, current_servers.clone()));
                        current_servers.clear();
                    }
                }
                
                let repo_name = line.trim_start_matches('[').trim_end_matches(']');
                // Skip [options] section
                if repo_name != "options" {
                    current_repo = Some(repo_name.to_string());
                }
            }
            // Check for Server or Include directives
            else if let Some(repo) = &current_repo {
                if line.starts_with("Server") {
                    if let Some(url) = line.split('=').nth(1) {
                        let url = url.trim();
                        // Replace $repo and $arch variables
                        let url = url.replace("$repo", repo).replace("$arch", "x86_64");
                        current_servers.push(url);
                    }
                } else if line.starts_with("Include") {
                    // Parse included mirrorlist files
                    if let Some(path) = line.split('=').nth(1) {
                        let path = path.trim();
                        if let Ok(content) = fs::read_to_string(path) {
                            for mirror_line in content.lines() {
                                let mirror_line = mirror_line.trim();
                                if mirror_line.starts_with("Server") {
                                    if let Some(url) = mirror_line.split('=').nth(1) {
                                        let url = url.trim();
                                        let url = url.replace("$repo", repo).replace("$arch", "x86_64");
                                        current_servers.push(url);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Save last repo
        if let Some(repo) = current_repo {
            if !current_servers.is_empty() {
                repos.push((repo, current_servers));
            }
        }
        
        // If no repos found, fallback to standard repos
        if repos.is_empty() {
            repos = vec![
                ("core".to_string(), vec!["https://geo.mirror.pkgbuild.com/core/os/x86_64".to_string()]),
                ("extra".to_string(), vec!["https://geo.mirror.pkgbuild.com/extra/os/x86_64".to_string()]),
                ("multilib".to_string(), vec!["https://geo.mirror.pkgbuild.com/multilib/os/x86_64".to_string()]),
            ];
        }
        
        Ok(repos)
    }
    
    /// Sync databases using ALL available mirrors with failover
    pub async fn sync_dbs_manual(&mut self, mp: Option<MultiProgress>, concurrency: usize) -> Result<()> {
        let mut download_targets = Vec::new();
        let sync_dir = Path::new(&self.dbpath).join("sync");
        
        // Collect ALL mirrors for each database for racing/failover
        for db in self.handle.syncdbs() {
            let servers: Vec<String> = db.servers()
                .iter()
                .map(|s| format!("{}/{}.db", s, db.name()))
                .collect();
            
            if !servers.is_empty() {
                download_targets.push((servers, format!("{}.db", db.name())));
            } else {
                // Fallback to geo mirror if no servers registered
                let fallback = vec![format!(
                    "https://geo.mirror.pkgbuild.com/{}/os/x86_64/{}.db",
                    db.name(),
                    db.name()
                )];
                download_targets.push((fallback, format!("{}.db", db.name())));
            }
        }
        
        crate::downloader::download_packages(download_targets, &sync_dir, mp, concurrency).await?;
        Ok(())
    }
    
    /// Get all mirror URLs for a specific repository
    pub fn get_repo_mirrors(&self, repo_name: &str) -> Vec<String> {
        for db in self.handle.syncdbs() {
            if db.name() == repo_name {
                return db.servers().iter().map(|s| s.to_string()).collect();
            }
        }
        // Fallback
        vec![format!("https://geo.mirror.pkgbuild.com/{}/os/x86_64", repo_name)]
    }

    pub fn search(&self, queries: Vec<String>) -> Result<Vec<&alpm::Package>> {
        let mut results = Vec::new();
        let sync_dbs = self.handle.syncdbs();
        
        // Simple OR search across all sync dbs
        for db in sync_dbs {
            let query_refs: Vec<&str> = queries.iter().map(|s| s.as_str()).collect();
            if let Ok(pkgs) = db.search(query_refs.into_iter()) {
                for pkg in pkgs {
                    results.push(pkg);
                }
            }
        }
        Ok(results)
    }
}