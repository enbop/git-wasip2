use std::{env, error::Error};

use git_wasip2::{FetchLimits, FetchRequest, Remote, fetch, push_one_commit};

fn main() -> Result<(), Box<dyn Error>> {
    rustls_rustcrypto::provider()
        .install_default()
        .map_err(|_| "failed to install the RustCrypto TLS provider")?;

    let mut arguments = env::args().skip(1);
    match arguments.next().as_deref() {
        Some("fetch") => {
            let remote_url = required(&mut arguments, "REMOTE_URL")?;
            let repository = required(&mut arguments, "REPOSITORY")?;
            let branch = required(&mut arguments, "BRANCH")?;
            ensure_finished(&mut arguments)?;
            let remote = Remote::new(remote_url, None)?;
            let request = FetchRequest::branch(repository, remote, branch);
            let outcome = runtime()?.block_on(fetch(request))?;
            println!("remote_tip={}", outcome.remote_tip);
            println!("remote_refs={}", outcome.remote_refs);
            println!("repository_bytes={}", outcome.repository_bytes);
        }
        Some("create-commit") => {
            let repository = required(&mut arguments, "REPOSITORY")?;
            let base_ref = required(&mut arguments, "BASE_REF")?;
            let candidate_ref = required(&mut arguments, "CANDIDATE_REF")?;
            let path = required(&mut arguments, "PATH")?;
            let content = required(&mut arguments, "CONTENT")?;
            ensure_finished(&mut arguments)?;
            create_commit(&repository, &base_ref, &candidate_ref, &path, &content)?;
        }
        Some("push") => {
            let remote_url = required(&mut arguments, "REMOTE_URL")?;
            let repository = required(&mut arguments, "REPOSITORY")?;
            let local_ref = required(&mut arguments, "LOCAL_REF")?;
            let remote_ref = required(&mut arguments, "REMOTE_REF")?;
            ensure_finished(&mut arguments)?;
            let remote = Remote::new(remote_url, None)?;
            let outcome = runtime()?.block_on(push_one_commit(
                remote,
                repository,
                &local_ref,
                &remote_ref,
                FetchLimits::default(),
            ))?;
            println!("previous_remote={}", outcome.previous_remote);
            println!("pushed_commit={}", outcome.pushed_commit);
            println!("remote_ref={}", outcome.remote_ref);
            println!("pack_bytes={}", outcome.pack_bytes);
        }
        _ => {
            return Err(
                "usage: git-wasip2-driver <fetch|create-commit|push> [arguments...]".into(),
            );
        }
    }
    Ok(())
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
        .ok_or_else(|| format!("missing required argument {name}").into())
}

fn ensure_finished(arguments: &mut impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    if let Some(argument) = arguments.next() {
        return Err(format!("unexpected argument {argument:?}").into());
    }
    Ok(())
}

fn create_commit(
    repository_path: &str,
    base_ref: &str,
    candidate_ref: &str,
    path: &str,
    content: &str,
) -> Result<(), Box<dyn Error>> {
    use gix::{bstr::ByteSlice, objs::tree::EntryKind};

    if !candidate_ref.starts_with("refs/") || candidate_ref.starts_with("refs/heads/") {
        return Err("candidate ref must be an internal ref outside refs/heads/".into());
    }
    let repository = gix::open(repository_path)?;
    let base = repository
        .find_reference(base_ref)?
        .into_fully_peeled_id()?;
    let base_commit = repository.find_commit(base)?;
    let mut editor = repository.edit_tree(base_commit.tree_id()?)?;
    let blob = repository.write_blob(content.as_bytes())?;
    let tree = editor.upsert(path, EntryKind::Blob, blob)?.write()?;
    let identity = gix::actor::SignatureRef {
        name: b"git-wasip2 integration test".as_bstr(),
        email: b"integration@git-wasip2.invalid".as_bstr(),
        time: "1784160000 +0000",
    };
    let commit = repository.commit_as(
        identity,
        identity,
        candidate_ref,
        "test: create WASIp2 push candidate",
        tree,
        [base.detach()],
    )?;
    println!("base={base}");
    println!("commit={commit}");
    println!("tree={tree}");
    Ok(())
}
