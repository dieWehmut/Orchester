//! Computer project catalog and read-only browsing inside configured roots.
use super::*;
use axum::extract::Query;
use std::path::Path;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}
impl Project {
    pub fn from_path(path: PathBuf) -> Self {
        Self {
            id: hash(&path.to_string_lossy()),
            name: path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            path,
        }
    }
}
#[derive(Deserialize, Default)]
pub(super) struct ProjectQuery {
    pub project_id: Option<String>,
}
impl RemoteControl {
    pub(super) fn project(&self, id: Option<&str>) -> Result<Project, StatusCode> {
        let id = id.unwrap_or(&self.0.default_project_id);
        self.0
            .projects
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or(StatusCode::NOT_FOUND)
    }
    pub(super) fn checked_directory(&self, path: &Path) -> Result<PathBuf, StatusCode> {
        if !path.is_absolute() {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
        let actual = path.canonicalize().map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
            _ => StatusCode::NOT_FOUND,
        })?;
        if !actual.is_dir() {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
        if !self
            .0
            .allowed_roots
            .iter()
            .any(|root| actual.starts_with(root))
        {
            return Err(StatusCode::FORBIDDEN);
        }
        Ok(actual)
    }
    pub(super) fn validate_project(&self, project: &Project) -> Result<(), StatusCode> {
        if self.checked_directory(&project.path)? != project.path {
            return Err(StatusCode::CONFLICT);
        }
        Ok(())
    }
    pub(super) fn project_registry(&self, project: &Project) -> orchester_verzeichnis::Registry {
        if project.id == self.0.default_project_id {
            return self.0.context.registry();
        }
        let paths = self
            .0
            .context
            .paths()
            .unwrap()
            .with_workspace(&project.path);
        match orchester_verzeichnis::standard_plugin_roots(paths.home(), &project.path) {
            Ok(roots) => orchester_verzeichnis::Registry::discover_with_plugin_roots(
                paths.manifest_dir(),
                roots,
            ),
            Err(_) => orchester_verzeichnis::Registry::discover(paths.manifest_dir()),
        }
    }
}
pub(super) async fn list_projects(
    State(s): State<RemoteControl>,
    h: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    s.authorize(&h)?;
    let mut projects: Vec<_> = s.0.projects.lock().unwrap().values().cloned().collect();
    projects.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Json(json!({"projects":projects})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AddProject {
    path: PathBuf,
    name: Option<String>,
}
pub(super) async fn add_project(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Json(input): Json<AddProject>,
) -> Result<Json<Project>, StatusCode> {
    s.authorize(&h)?;
    if input
        .name
        .as_ref()
        .is_some_and(|name| name.trim().is_empty() || name.len() > 128)
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let reader = s.clone();
    let path = tokio::task::spawn_blocking(move || reader.checked_directory(&input.path))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;
    let mut project = Project::from_path(path);
    if let Some(name) = input.name {
        project.name = name.trim().to_owned();
    }
    let mut projects = s.0.projects.lock().unwrap();
    if let Some(existing) = projects.get(&project.id) {
        return Ok(Json(existing.clone()));
    }
    if projects.len() >= 128 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let mut next = projects.clone();
    next.insert(project.id.clone(), project.clone());
    // Match the established runs -> database lock order while importing v1
    // history for a workspace explicitly selected for the first time.
    let mut runs = s.0.runs.lock().unwrap();
    let mut db = s.0.database.lock().unwrap();
    let transaction = db
        .transaction()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let imported = {
        let mut statement = transaction
            .prepare("SELECT id,value FROM remote_runs WHERE workspace=?1")
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let rows = statement
            .query_map([project.path.to_string_lossy().as_ref()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let mut imported = Vec::new();
        for row in rows {
            let (id, value) = row.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let mut run: Run =
                serde_json::from_str(&value).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if runs.contains_key(&id) {
                continue;
            }
            run.summary["project_id"] = json!(project.id);
            run.summary["workspace"] = json!(project.path);
            if run.summary["state"] == "running" {
                run.summary["state"] = json!("interrupted");
            }
            imported.push((id, run));
        }
        imported
    };
    for (id, run) in &imported {
        write_run(&transaction, &project.path.to_string_lossy(), id, run)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    write_value(&transaction, "projects", &next).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    transaction
        .commit()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    drop(db);
    for (id, run) in imported {
        runs.insert(id, run);
    }
    *projects = next;
    Ok(Json(project))
}
#[derive(Deserialize)]
pub(super) struct FolderQuery {
    path: Option<PathBuf>,
    #[serde(default)]
    offset: usize,
    #[serde(default = "page_size")]
    limit: usize,
}
fn page_size() -> usize {
    200
}
pub(super) async fn folders(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Query(q): Query<FolderQuery>,
) -> Result<Json<Value>, StatusCode> {
    s.authorize(&h)?;
    if q.limit == 0 || q.limit > 200 || q.offset > 10000 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    tokio::task::spawn_blocking(move || {
        let path=s.checked_directory(q.path.as_deref().unwrap_or(&s.0.workspace))?;
        let mut entries=Vec::new();
        let iterator=std::fs::read_dir(&path).map_err(|_|StatusCode::FORBIDDEN)?;
        for (index,entry) in iterator.enumerate() {
            if index>=10000 { return Err(StatusCode::PAYLOAD_TOO_LARGE); }
            let Ok(entry)=entry else {continue};
            let Ok(kind)=entry.file_type() else {continue};
            if kind.is_symlink() || !(kind.is_dir() || kind.is_file()) {continue;}
            let Ok(actual)=entry.path().canonicalize() else {continue};
            if !s.0.allowed_roots.iter().any(|root|actual.starts_with(root)) {continue;}
            let Ok(name)=entry.file_name().into_string() else {continue;};
            if actual.to_str().is_none() {continue;}
            entries.push((kind.is_dir(),name,actual));
        }
        entries.sort_by(|a,b| b.0.cmp(&a.0).then_with(||a.1.cmp(&b.1)));
        let total=entries.len();
        let page:Vec<_>=entries.into_iter().skip(q.offset).take(q.limit).map(|(directory,name,path)|json!({"name":name,"path":path,"type":if directory {"directory"} else {"file"}})).collect();
        let next_offset=(q.offset+page.len()<total).then_some(q.offset+page.len());
        let parent=path.parent().filter(|parent|s.0.allowed_roots.iter().any(|root|parent.starts_with(root)));
        Ok(Json(json!({"path":path,"parent_path":parent,"roots":s.0.allowed_roots,"entries":page,"next_offset":next_offset})))
    }).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?
}
