use std::{
    env,
    error::Error,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use git_wasip2::{
    Credentials, FetchLimits, FetchRequest, Remote, SnapshotCommitRequest, create_snapshot_commit,
    directory_changed_paths, export_full_snapshot, fetch, push_one_commit, reference_oid,
};

fn main() -> Result<(), Box<dyn Error>> {
    rustls_rustcrypto::provider()
        .install_default()
        .map_err(|_| "failed to install the RustCrypto TLS provider")?;

    let mut arguments = env::args().skip(1);
    match arguments.next().as_deref() {
        Some("fetch") => fetch_command(&mut arguments),
        Some("checkout") => checkout_command(&mut arguments),
        Some("status") => status_command(&mut arguments),
        Some("commit") => commit_command(&mut arguments),
        Some("push") => push_command(&mut arguments),
        Some("show-ref") => show_ref_command(&mut arguments),
        _ => Err(usage().into()),
    }
}

fn fetch_command(arguments: &mut impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let remote_url = required(arguments, "REMOTE_URL")?;
    let repository = required(arguments, "REPOSITORY")?;
    let branch = required(arguments, "BRANCH")?;
    ensure_finished(arguments)?;
    let request = FetchRequest::branch(repository, remote(&remote_url)?, branch);
    let outcome = runtime()?.block_on(fetch(request))?;
    println!("remote_tip={}", outcome.remote_tip);
    println!("remote_refs={}", outcome.remote_refs);
    println!("repository_bytes={}", outcome.repository_bytes);
    Ok(())
}

fn checkout_command(arguments: &mut impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let repository = required(arguments, "REPOSITORY")?;
    let revision = required(arguments, "REVISION")?;
    let destination = required(arguments, "DESTINATION")?;
    ensure_finished(arguments)?;
    let commit = resolve_revision(&repository, &revision)?;
    export_full_snapshot(&repository, &commit, &destination)?;
    println!("commit={commit}");
    println!("worktree={destination}");
    Ok(())
}

fn status_command(arguments: &mut impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let repository = required(arguments, "REPOSITORY")?;
    let revision = required(arguments, "REVISION")?;
    let worktree = required(arguments, "WORKTREE")?;
    ensure_finished(arguments)?;
    let commit = resolve_revision(&repository, &revision)?;
    let changed = directory_changed_paths(&repository, &commit, &worktree)?;
    if changed.is_empty() {
        println!("clean");
    } else {
        for path in changed {
            println!("changed={path}");
        }
    }
    Ok(())
}

fn commit_command(arguments: &mut impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let repository = required(arguments, "REPOSITORY")?;
    let parent = required(arguments, "PARENT")?;
    let worktree = required(arguments, "WORKTREE")?;
    let candidate_ref = required(arguments, "CANDIDATE_REF")?;
    let message = required(arguments, "MESSAGE")?;
    ensure_finished(arguments)?;
    let parent = resolve_revision(&repository, &parent)?;
    let request = SnapshotCommitRequest {
        repository: PathBuf::from(repository),
        parent,
        source_directory: PathBuf::from(worktree),
        candidate_ref: candidate_ref.clone(),
        committed_at_unix: i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())?,
        author_name: env::var("GIT_WASIP2_AUTHOR_NAME")
            .unwrap_or_else(|_| "git-wasip2 user".to_owned()),
        author_email: env::var("GIT_WASIP2_AUTHOR_EMAIL")
            .unwrap_or_else(|_| "user@git-wasip2.invalid".to_owned()),
        message,
    };
    match create_snapshot_commit(request)? {
        Some(outcome) => {
            println!("commit={}", outcome.commit);
            println!("tree={}", outcome.tree);
            println!("candidate_ref={candidate_ref}");
            for path in outcome.changed_paths {
                println!("changed={path}");
            }
        }
        None => println!("clean"),
    }
    Ok(())
}

fn push_command(arguments: &mut impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let remote_url = required(arguments, "REMOTE_URL")?;
    let repository = required(arguments, "REPOSITORY")?;
    let local_ref = required(arguments, "LOCAL_REF")?;
    let remote_ref = required(arguments, "REMOTE_REF")?;
    ensure_finished(arguments)?;
    let outcome = runtime()?.block_on(push_one_commit(
        remote(&remote_url)?,
        repository,
        &local_ref,
        &remote_ref,
        FetchLimits::default(),
    ))?;
    println!("previous_remote={}", outcome.previous_remote);
    println!("pushed_commit={}", outcome.pushed_commit);
    println!("remote_ref={}", outcome.remote_ref);
    println!("pack_bytes={}", outcome.pack_bytes);
    Ok(())
}

fn show_ref_command(arguments: &mut impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let repository = required(arguments, "REPOSITORY")?;
    let reference = required(arguments, "REFERENCE")?;
    ensure_finished(arguments)?;
    println!("{}", reference_oid(repository, &reference)?);
    Ok(())
}

fn resolve_revision(repository: &str, revision: &str) -> Result<String, Box<dyn Error>> {
    if revision.starts_with("refs/") {
        Ok(reference_oid(repository, revision)?)
    } else {
        Ok(revision.to_owned())
    }
}

fn remote(url: &str) -> Result<Remote, Box<dyn Error>> {
    let username = env::var("GIT_WASIP2_USERNAME").ok();
    let password = env::var("GIT_WASIP2_PASSWORD").ok();
    let credentials = match (username, password) {
        (None, None) => None,
        (Some(username), Some(password)) => Some(Credentials::basic(username, password)),
        _ => {
            return Err("GIT_WASIP2_USERNAME and GIT_WASIP2_PASSWORD must be set together".into());
        }
    };
    Ok(Remote::new(url, credentials)?)
}

fn runtime() -> Result<tokio::runtime::Runtime, Box<dyn Error>> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}

fn required(
    arguments: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, Box<dyn Error>> {
    arguments
        .next()
        .ok_or_else(|| format!("missing required argument {name}\n\n{}", usage()).into())
}

fn ensure_finished(arguments: &mut impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    if let Some(argument) = arguments.next() {
        return Err(format!("unexpected argument {argument:?}\n\n{}", usage()).into());
    }
    Ok(())
}

fn usage() -> &'static str {
    "usage:
  git-wasip2 fetch REMOTE_URL REPOSITORY BRANCH
  git-wasip2 checkout REPOSITORY REVISION DESTINATION
  git-wasip2 status REPOSITORY REVISION WORKTREE
  git-wasip2 commit REPOSITORY PARENT WORKTREE CANDIDATE_REF MESSAGE
  git-wasip2 push REMOTE_URL REPOSITORY LOCAL_REF REMOTE_REF
  git-wasip2 show-ref REPOSITORY REFERENCE"
}
