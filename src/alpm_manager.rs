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
use indicatif::{MultiProgress};

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
        
        for repo in ["core", "extra", "multilib"].iter() {
            let db = handle.register_syncdb_mut(*repo, SigLevel::DATABASE_OPTIONAL)?;
            let url = format!("https://geo.mirror.pkgbuild.com/{}/os/x86_64", repo);
            db.add_server(url.as_str())?; 
        }

        Ok(Self { 
            handle,
            dbpath: dbpath.to_string(),
        })
    }
    
    pub async fn sync_dbs_manual(&mut self, mp: Option<MultiProgress>, concurrency: usize) -> Result<()> {
        let mut download_targets = Vec::new();
        let sync_dir = Path::new(&self.dbpath).join("sync");
        
        // Try to get mirrors from the registered dbs
        for db in self.handle.syncdbs() {
            if let Some(server) = db.servers().first() {
                let url = format!("{}/{}.db", server, db.name());
                download_targets.push((url, format!("{}.db", db.name())));
            } else {
                // Fallback to geo mirror if no server is registered
                let url = format!("https://geo.mirror.pkgbuild.com/{}/os/x86_64/{}.db", db.name(), db.name());
                download_targets.push((url, format!("{}.db", db.name())));
            }
        }
        
        crate::downloader::download_packages(download_targets, &sync_dir, mp, concurrency).await?;
        Ok(())
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