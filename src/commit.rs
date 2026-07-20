use std::{fs, path::PathBuf};

use gix::{bstr::ByteSlice, objs::tree::EntryKind};

use crate::Error;

#[derive(Clone, Debug)]
pub struct DirectoryCommitRequest {
    pub repository: PathBuf,
    pub parent: String,
    pub source_directory: PathBuf,
    pub root_entry: String,
    pub candidate_ref: String,
    pub committed_at_unix: i64,
    pub author_name: String,
    pub author_email: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryCommit {
    pub commit: String,
    pub tree: String,
    pub entry_tree: String,
    pub changed_paths: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SnapshotCommitRequest {
    pub repository: PathBuf,
    pub parent: String,
    pub source_directory: PathBuf,
    pub candidate_ref: String,
    pub committed_at_unix: i64,
    pub author_name: String,
    pub author_email: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotCommit {
    pub commit: String,
    pub tree: String,
    pub changed_paths: Vec<String>,
}

/// Create a one-parent commit by replacing one root tree entry with a directory.
///
/// The candidate is written to an internal reference, never directly to a
/// branch. The caller can inspect it and use [`crate::push_one_commit`] for a
/// compare-and-swap style publication.
pub fn create_directory_commit(
    request: DirectoryCommitRequest,
) -> Result<Option<DirectoryCommit>, Error> {
    validate_request(&request)?;
    let repository = gix::open(&request.repository).map_err(git_error)?;
    let parent_oid = gix::hash::ObjectId::from_hex(request.parent.as_bytes()).map_err(git_error)?;
    let parent_commit = repository.find_commit(parent_oid).map_err(git_error)?;
    let parent_tree = parent_commit.tree().map_err(git_error)?;
    let mut entry_editor = repository
        .edit_tree(repository.empty_tree().id)
        .map_err(git_error)?;
    let mut changed_paths = Vec::new();
    add_directory_files(
        &repository,
        &request.source_directory,
        &request.source_directory,
        &mut entry_editor,
        &mut changed_paths,
    )?;
    changed_paths.sort_unstable();
    let entry_tree = entry_editor.write().map_err(git_error)?;
    if parent_tree
        .find_entry(request.root_entry.as_str())
        .is_some_and(|entry| entry.object_id() == entry_tree.detach())
    {
        return Ok(None);
    }

    let mut root_editor = repository.edit_tree(parent_tree.id()).map_err(git_error)?;
    let tree = root_editor
        .upsert(request.root_entry.as_str(), EntryKind::Tree, entry_tree)
        .map_err(git_error)?
        .write()
        .map_err(git_error)?;
    let time = format!("{} +0000", request.committed_at_unix);
    let identity = gix::actor::SignatureRef {
        name: request.author_name.as_bytes().as_bstr(),
        email: request.author_email.as_bytes().as_bstr(),
        time: &time,
    };
    let commit = repository
        .commit_as(
            identity,
            identity,
            request.candidate_ref.as_str(),
            request.message,
            tree,
            [parent_oid],
        )
        .map_err(git_error)?;

    Ok(Some(DirectoryCommit {
        commit: commit.to_string(),
        tree: tree.to_string(),
        entry_tree: entry_tree.to_string(),
        changed_paths: changed_paths
            .into_iter()
            .map(|path| format!("{}/{path}", request.root_entry))
            .collect(),
    }))
}

/// Create a one-parent commit from the complete contents of a directory.
///
/// The source directory is a standalone worktree and must not contain Git
/// metadata, symbolic links, or non-file entries. The candidate is written to
/// an internal reference, never directly to a branch.
pub fn create_snapshot_commit(
    request: SnapshotCommitRequest,
) -> Result<Option<SnapshotCommit>, Error> {
    validate_candidate_and_identity(
        &request.candidate_ref,
        &request.author_name,
        &request.author_email,
    )?;
    let repository = gix::open(&request.repository).map_err(git_error)?;
    let parent_oid = gix::hash::ObjectId::from_hex(request.parent.as_bytes()).map_err(git_error)?;
    let parent_tree = repository
        .find_commit(parent_oid)
        .map_err(git_error)?
        .tree_id()
        .map_err(git_error)?;
    let mut editor = repository
        .edit_tree(repository.empty_tree().id)
        .map_err(git_error)?;
    let mut source_paths = Vec::new();
    add_directory_files(
        &repository,
        &request.source_directory,
        &request.source_directory,
        &mut editor,
        &mut source_paths,
    )?;
    let tree = editor.write().map_err(git_error)?;
    if tree == parent_tree {
        return Ok(None);
    }

    let time = format!("{} +0000", request.committed_at_unix);
    let identity = gix::actor::SignatureRef {
        name: request.author_name.as_bytes().as_bstr(),
        email: request.author_email.as_bytes().as_bstr(),
        time: &time,
    };
    let commit = repository
        .commit_as(
            identity,
            identity,
            request.candidate_ref.as_str(),
            request.message,
            tree,
            [parent_oid],
        )
        .map_err(git_error)?;
    let changed_paths = crate::changed_paths(
        &request.repository,
        Some(&request.parent),
        &commit.to_string(),
    )?;

    Ok(Some(SnapshotCommit {
        commit: commit.to_string(),
        tree: tree.to_string(),
        changed_paths,
    }))
}

fn validate_request(request: &DirectoryCommitRequest) -> Result<(), Error> {
    validate_candidate_and_identity(
        &request.candidate_ref,
        &request.author_name,
        &request.author_email,
    )?;
    if request.root_entry.is_empty()
        || request.root_entry.contains(['/', '\0'])
        || matches!(request.root_entry.as_str(), "." | "..")
    {
        return Err(Error::Git(format!(
            "invalid root tree entry {:?}",
            request.root_entry
        )));
    }
    Ok(())
}

fn validate_candidate_and_identity(
    candidate_ref: &str,
    author_name: &str,
    author_email: &str,
) -> Result<(), Error> {
    if !candidate_ref.starts_with("refs/") || candidate_ref.starts_with("refs/heads/") {
        return Err(Error::Git(format!(
            "candidate ref must be an internal ref outside refs/heads/: {candidate_ref:?}"
        )));
    }
    gix::refs::FullName::try_from(candidate_ref).map_err(git_error)?;
    if author_name.is_empty() || author_email.is_empty() {
        return Err(Error::Git("commit identity must not be empty".to_owned()));
    }
    Ok(())
}

fn add_directory_files(
    repository: &gix::Repository,
    source_root: &std::path::Path,
    directory: &std::path::Path,
    editor: &mut gix::object::tree::Editor<'_>,
    changed_paths: &mut Vec<String>,
) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(directory).map_err(|source| Error::Io {
        path: directory.to_owned(),
        source,
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::Git(format!(
            "source path is not a real directory: {}",
            directory.display()
        )));
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|source| Error::Io {
            path: directory.to_owned(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| Error::Io {
            path: directory.to_owned(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if directory == source_root && entry.file_name() == ".git" {
            return Err(Error::Git(format!(
                "source directory must not contain Git metadata: {}",
                path.display()
            )));
        }
        let metadata = fs::symlink_metadata(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(Error::Git(format!(
                "symbolic links are not allowed in committed directories: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            add_directory_files(repository, source_root, &path, editor, changed_paths)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(Error::Git(format!(
                "unsupported filesystem entry: {}",
                path.display()
            )));
        }
        let relative = path.strip_prefix(source_root).map_err(git_error)?;
        let relative = relative
            .to_str()
            .ok_or_else(|| Error::Git(format!("path is not UTF-8: {}", path.display())))?;
        let data = fs::read(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        let blob = repository.write_blob(&data).map_err(git_error)?;
        editor
            .upsert(relative, EntryKind::Blob, blob)
            .map_err(git_error)?;
        changed_paths.push(relative.to_owned());
    }
    Ok(())
}

fn git_error(error: impl std::fmt::Display + std::fmt::Debug) -> Error {
    Error::Git(format!("{error:#?}"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use gix::{bstr::ByteSlice, objs::tree::EntryKind};

    use super::{
        DirectoryCommitRequest, SnapshotCommitRequest, create_directory_commit,
        create_snapshot_commit,
    };

    #[test]
    fn candidate_replaces_only_the_selected_root_entry() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let repository = gix::init(temporary.path().join("repository")).expect("repository");
        let identity = gix::actor::SignatureRef {
            name: b"git-wasip2 test".as_bstr(),
            email: b"test@git-wasip2.invalid".as_bstr(),
            time: "1784160000 +0000",
        };
        let content = repository.write_blob(b"content\n").expect("content blob");
        let old_data = repository.write_blob(b"old\n").expect("old data blob");
        let mut editor = repository
            .edit_tree(repository.empty_tree().id)
            .expect("tree editor");
        editor
            .upsert("content/item.md", EntryKind::Blob, content)
            .expect("content entry");
        editor
            .upsert("data/item.toml", EntryKind::Blob, old_data)
            .expect("old data entry");
        let tree = editor.write().expect("base tree");
        let base = repository
            .commit_as(
                identity,
                identity,
                "refs/heads/main",
                "base",
                tree,
                gix::commit::NO_PARENT_IDS,
            )
            .expect("base commit");
        let source = temporary.path().join("data");
        fs::create_dir_all(&source).expect("source directory");
        fs::write(source.join("item.toml"), b"new\n").expect("source file");

        let candidate = create_directory_commit(DirectoryCommitRequest {
            repository: repository.path().to_owned(),
            parent: base.to_string(),
            source_directory: source,
            root_entry: "data".to_owned(),
            candidate_ref: "refs/git-wasip2/candidate".to_owned(),
            committed_at_unix: 1_784_160_000,
            author_name: "git-wasip2".to_owned(),
            author_email: "sync@git-wasip2.invalid".to_owned(),
            message: "test: update data".to_owned(),
        })
        .expect("directory commit")
        .expect("changed directory");

        let commit = repository
            .find_commit(
                gix::hash::ObjectId::from_hex(candidate.commit.as_bytes())
                    .expect("candidate object ID"),
            )
            .expect("candidate commit");
        assert_eq!(
            commit
                .parent_ids()
                .map(|parent| parent.to_string())
                .collect::<Vec<_>>(),
            [base.to_string()]
        );
        assert_eq!(candidate.changed_paths, ["data/item.toml"]);
    }

    #[test]
    fn snapshot_candidate_tracks_additions_modifications_and_deletions() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let repository = gix::init(temporary.path().join("repository")).expect("repository");
        let identity = gix::actor::SignatureRef {
            name: b"git-wasip2 test".as_bstr(),
            email: b"test@git-wasip2.invalid".as_bstr(),
            time: "1784160000 +0000",
        };
        let unchanged = repository.write_blob(b"unchanged\n").expect("blob");
        let changed = repository.write_blob(b"before\n").expect("blob");
        let deleted = repository.write_blob(b"deleted\n").expect("blob");
        let mut editor = repository
            .edit_tree(repository.empty_tree().id)
            .expect("tree editor");
        editor
            .upsert("unchanged.txt", EntryKind::Blob, unchanged)
            .expect("entry");
        editor
            .upsert("changed.txt", EntryKind::Blob, changed)
            .expect("entry");
        editor
            .upsert("deleted.txt", EntryKind::Blob, deleted)
            .expect("entry");
        let tree = editor.write().expect("tree");
        let base = repository
            .commit_as(
                identity,
                identity,
                "refs/heads/main",
                "base",
                tree,
                gix::commit::NO_PARENT_IDS,
            )
            .expect("base commit");
        let source = temporary.path().join("worktree");
        fs::create_dir(&source).expect("worktree");
        fs::write(source.join("unchanged.txt"), b"unchanged\n").expect("unchanged");
        fs::write(source.join("changed.txt"), b"after\n").expect("changed");
        fs::write(source.join("added.txt"), b"added\n").expect("added");

        let candidate = create_snapshot_commit(SnapshotCommitRequest {
            repository: repository.path().to_owned(),
            parent: base.to_string(),
            source_directory: source,
            candidate_ref: "refs/git-wasip2/candidate".to_owned(),
            committed_at_unix: 1_784_160_000,
            author_name: "git-wasip2".to_owned(),
            author_email: "test@git-wasip2.invalid".to_owned(),
            message: "test: snapshot".to_owned(),
        })
        .expect("snapshot commit")
        .expect("changed snapshot");

        assert_eq!(
            candidate.changed_paths,
            ["added.txt", "changed.txt", "deleted.txt"]
        );
        let commit = repository
            .find_commit(
                gix::hash::ObjectId::from_hex(candidate.commit.as_bytes()).expect("commit ID"),
            )
            .expect("candidate commit");
        assert_eq!(
            commit
                .parent_ids()
                .map(|parent| parent.to_string())
                .collect::<Vec<_>>(),
            [base.to_string()]
        );
    }

    #[test]
    fn snapshot_candidate_rejects_a_git_metadata_entry() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let repository = gix::init(temporary.path().join("repository")).expect("repository");
        let identity = gix::actor::SignatureRef {
            name: b"git-wasip2 test".as_bstr(),
            email: b"test@git-wasip2.invalid".as_bstr(),
            time: "1784160000 +0000",
        };
        let base = repository
            .commit_as(
                identity,
                identity,
                "refs/heads/main",
                "base",
                repository.empty_tree().id,
                gix::commit::NO_PARENT_IDS,
            )
            .expect("base commit");
        let source = temporary.path().join("worktree");
        fs::create_dir(&source).expect("worktree");
        fs::write(source.join(".git"), b"gitdir: elsewhere\n").expect("metadata entry");

        assert!(
            create_snapshot_commit(SnapshotCommitRequest {
                repository: repository.path().to_owned(),
                parent: base.to_string(),
                source_directory: source,
                candidate_ref: "refs/git-wasip2/candidate".to_owned(),
                committed_at_unix: 1_784_160_000,
                author_name: "git-wasip2".to_owned(),
                author_email: "test@git-wasip2.invalid".to_owned(),
                message: "test: snapshot".to_owned(),
            })
            .is_err()
        );
    }
}
