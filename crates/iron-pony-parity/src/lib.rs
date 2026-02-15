use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use iron_pony_spec::RequirementSpec;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ParityConfig {
    pub workspace_root: PathBuf,
    pub cases_dir: PathBuf,
    pub spec_path: PathBuf,
    pub output_dir: PathBuf,
    pub reference_program: String,
    pub candidate_program: Option<PathBuf>,
}

impl ParityConfig {
    pub fn default_for_workspace(workspace_root: impl AsRef<Path>) -> Self {
        let workspace_root = workspace_root.as_ref().to_path_buf();
        Self {
            cases_dir: workspace_root.join("tests/parity_cases"),
            spec_path: workspace_root.join("spec/requirements.yaml"),
            output_dir: workspace_root.join("target/parity"),
            reference_program: std::env::var("PONYSAY_REF")
                .unwrap_or_else(|_| "ponysay".to_string()),
            candidate_program: std::env::var("IRON_PONY_BIN").ok().map(PathBuf::from),
            workspace_root,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParityCase {
    pub id: String,
    #[serde(default)]
    pub features: Vec<String>,
    pub argv: Vec<String>,
    #[serde(default)]
    pub reference_program: Option<String>,
    #[serde(default)]
    pub reference_argv: Option<Vec<String>>,
    #[serde(default)]
    pub candidate_program: Option<String>,
    #[serde(default)]
    pub candidate_argv: Option<Vec<String>>,
    #[serde(default)]
    pub stdin: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseResult {
    pub id: String,
    pub features: Vec<String>,
    pub passed: bool,
    pub exit_match: bool,
    pub stdout_match: bool,
    pub stderr_match: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementResult {
    pub id: String,
    pub weight: f64,
    pub covered_cases: usize,
    pub passing_cases: usize,
    pub score: f64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportSummary {
    pub total_cases: usize,
    pub passed_cases: usize,
    pub case_parity: f64,
    pub weighted_requirement_parity: f64,
    pub requirement_completion: f64,
    pub untested_requirements: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParityReport {
    pub generated_epoch_secs: u64,
    pub summary: ReportSummary,
    pub requirements: Vec<RequirementResult>,
    pub cases: Vec<CaseResult>,
}

#[derive(Debug, Clone)]
struct ProcessOutput {
    status_code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[derive(Debug, Default, Clone)]
struct RequirementAgg {
    weight: f64,
    covered: usize,
    passed: usize,
}

pub fn run_parity(config: &ParityConfig) -> Result<ParityReport> {
    info!(cases_dir = %config.cases_dir.display(), reference = %config.reference_program, "starting parity run");
    let spec = RequirementSpec::load(&config.spec_path)?;
    let cases = load_cases(&config.cases_dir)?;
    if cases.is_empty() {
        warn!("no parity cases found");
    }

    std::fs::create_dir_all(config.output_dir.join("failures"))
        .context("failed creating parity output directories")?;

    let mut case_results = Vec::new();

    for case in cases {
        let result = run_case(config, &case)?;
        if !result.passed {
            let diff_path = config
                .output_dir
                .join("failures")
                .join(format!("{}.diff", case.id));
            std::fs::write(&diff_path, &result.detail)
                .with_context(|| format!("failed writing diff for case {}", case.id))?;
            debug!(case = %case.id, path = %diff_path.display(), "wrote parity failure diff");
        }
        case_results.push(result);
    }

    let requirements = compute_requirement_scores(&spec, &case_results);
    let summary = compute_summary(&requirements, &case_results);
    let report = ParityReport {
        generated_epoch_secs: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        summary,
        requirements,
        cases: case_results,
    };

    write_report_artifacts(config, &report)?;
    Ok(report)
}

fn load_cases(path: &Path) -> Result<Vec<ParityCase>> {
    let mut files = Vec::new();
    if !path.exists() {
        return Ok(files);
    }

    let mut entries = std::fs::read_dir(path)
        .with_context(|| format!("failed reading cases dir {}", path.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed collecting case dir entries")?;

    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let candidate = entry.path();
        if candidate.extension() != Some(OsStr::new("json")) {
            continue;
        }

        let raw = std::fs::read_to_string(&candidate)
            .with_context(|| format!("failed reading case file {}", candidate.display()))?;
        let parsed = serde_json::from_str::<ParityCase>(&raw)
            .with_context(|| format!("failed parsing case file {}", candidate.display()))?;
        files.push(parsed);
    }

    Ok(files)
}

fn run_case(config: &ParityConfig, case: &ParityCase) -> Result<CaseResult> {
    debug!(case = %case.id, "running parity case");

    let temp = tempfile::tempdir().context("failed creating parity temp dir")?;
    let temp_path = temp.path();

    let env = case
        .env
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                substitute_vars(value, temp_path, &config.workspace_root),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let reference_argv = normalize_argv(
        case.reference_argv
            .as_ref()
            .unwrap_or(&case.argv)
            .iter()
            .map(|arg| substitute_vars(arg, temp_path, &config.workspace_root))
            .collect::<Vec<_>>(),
    );

    let candidate_argv = normalize_argv(
        case.candidate_argv
            .as_ref()
            .unwrap_or(&case.argv)
            .iter()
            .map(|arg| substitute_vars(arg, temp_path, &config.workspace_root))
            .collect::<Vec<_>>(),
    );

    let stdin = case
        .stdin
        .as_ref()
        .map(|value| substitute_vars(value, temp_path, &config.workspace_root));

    let reference_program = case
        .reference_program
        .as_deref()
        .unwrap_or(&config.reference_program);

    let reference = match run_process(
        reference_program,
        &reference_argv,
        &env,
        stdin.as_deref(),
        &config.workspace_root,
    ) {
        Ok(output) => output,
        Err(error) => {
            return Ok(CaseResult {
                id: case.id.clone(),
                features: case.features.clone(),
                passed: false,
                exit_match: false,
                stdout_match: false,
                stderr_match: false,
                detail: format!("reference command failed: {error:#}"),
            });
        }
    };

    let candidate = match run_candidate(
        config,
        case.candidate_program.as_deref(),
        &candidate_argv,
        &env,
        stdin.as_deref(),
    ) {
        Ok(output) => output,
        Err(error) => {
            return Ok(CaseResult {
                id: case.id.clone(),
                features: case.features.clone(),
                passed: false,
                exit_match: false,
                stdout_match: false,
                stderr_match: false,
                detail: format!("candidate command failed: {error:#}"),
            });
        }
    };

    let exit_match = reference.status_code == candidate.status_code;
    let stdout_match = reference.stdout == candidate.stdout;
    let stderr_match = reference.stderr == candidate.stderr;
    let passed = exit_match && stdout_match && stderr_match;

    let detail = build_case_detail(
        case,
        &reference,
        &candidate,
        exit_match,
        stdout_match,
        stderr_match,
    );

    Ok(CaseResult {
        id: case.id.clone(),
        features: case.features.clone(),
        passed,
        exit_match,
        stdout_match,
        stderr_match,
        detail,
    })
}

fn run_candidate(
    config: &ParityConfig,
    case_program: Option<&str>,
    argv: &[String],
    env: &BTreeMap<String, String>,
    stdin: Option<&str>,
) -> Result<ProcessOutput> {
    if let Some(program) = case_program {
        return run_process(program, argv, env, stdin, &config.workspace_root);
    }

    if let Some(program) = &config.candidate_program {
        return run_process(program, argv, env, stdin, &config.workspace_root);
    }

    let mut cargo_args = vec![
        "run".to_string(),
        "--quiet".to_string(),
        "-p".to_string(),
        "iron-pony-cli".to_string(),
        "--bin".to_string(),
        "iron-pony".to_string(),
        "--".to_string(),
    ];
    cargo_args.extend(argv.iter().cloned());

    run_process("cargo", &cargo_args, env, stdin, &config.workspace_root)
}

fn run_process(
    program: impl AsRef<OsStr>,
    argv: &[String],
    env: &BTreeMap<String, String>,
    stdin: Option<&str>,
    cwd: &Path,
) -> Result<ProcessOutput> {
    let mut command = Command::new(program.as_ref());
    command
        .args(argv)
        .envs(env)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().with_context(|| {
        format!(
            "failed to spawn process {}",
            program.as_ref().to_string_lossy()
        )
    })?;

    if let Some(stdin) = stdin {
        if let Some(mut input) = child.stdin.take() {
            input
                .write_all(stdin.as_bytes())
                .with_context(|| "failed to write process stdin")?;
        }
    }

    let output = child
        .wait_with_output()
        .with_context(|| "failed waiting for process output")?;

    Ok(ProcessOutput {
        status_code: output.status.code().unwrap_or(-1),
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn normalize_argv(mut argv: Vec<String>) -> Vec<String> {
    if argv
        .first()
        .map(|first| !first.starts_with('-'))
        .unwrap_or(false)
    {
        argv.remove(0);
    }
    argv
}

fn build_case_detail(
    case: &ParityCase,
    reference: &ProcessOutput,
    candidate: &ProcessOutput,
    exit_match: bool,
    stdout_match: bool,
    stderr_match: bool,
) -> String {
    let mut detail = String::new();
    detail.push_str(&format!("case: {}\n", case.id));
    detail.push_str(&format!("exit_match: {exit_match}\n"));
    detail.push_str(&format!("stdout_match: {stdout_match}\n"));
    detail.push_str(&format!("stderr_match: {stderr_match}\n\n"));

    detail.push_str("=== reference (stdout) ===\n");
    detail.push_str(&String::from_utf8_lossy(&reference.stdout));
    detail.push_str("\n\n=== candidate (stdout) ===\n");
    detail.push_str(&String::from_utf8_lossy(&candidate.stdout));
    detail.push_str("\n\n=== reference (stderr) ===\n");
    detail.push_str(&String::from_utf8_lossy(&reference.stderr));
    detail.push_str("\n\n=== candidate (stderr) ===\n");
    detail.push_str(&String::from_utf8_lossy(&candidate.stderr));

    if !stdout_match {
        detail.push_str("\n\n=== first stdout mismatch ===\n");
        detail.push_str(&first_mismatch(
            &reference.stdout,
            &candidate.stdout,
            "reference",
            "candidate",
        ));
    }
    if !stderr_match {
        detail.push_str("\n\n=== first stderr mismatch ===\n");
        detail.push_str(&first_mismatch(
            &reference.stderr,
            &candidate.stderr,
            "reference",
            "candidate",
        ));
    }

    detail
}

fn first_mismatch(left: &[u8], right: &[u8], left_name: &str, right_name: &str) -> String {
    let min = left.len().min(right.len());
    for index in 0..min {
        if left[index] != right[index] {
            return format!(
                "byte {index}: {left_name}=0x{:02x}, {right_name}=0x{:02x}",
                left[index], right[index]
            );
        }
    }

    if left.len() != right.len() {
        format!(
            "length mismatch: {left_name}={} bytes, {right_name}={} bytes",
            left.len(),
            right.len()
        )
    } else {
        "outputs are identical".to_string()
    }
}

fn compute_requirement_scores(
    spec: &RequirementSpec,
    cases: &[CaseResult],
) -> Vec<RequirementResult> {
    let mut agg = BTreeMap::<String, RequirementAgg>::new();
    for requirement in &spec.requirements {
        agg.insert(
            requirement.id.clone(),
            RequirementAgg {
                weight: requirement.weight,
                ..RequirementAgg::default()
            },
        );
    }

    for case in cases {
        let mapped = map_requirements(spec, &case.features);
        for requirement in mapped {
            let entry = agg.entry(requirement).or_insert(RequirementAgg {
                weight: 1.0,
                ..RequirementAgg::default()
            });
            entry.covered += 1;
            if case.passed {
                entry.passed += 1;
            }
        }
    }

    let mut out = agg
        .into_iter()
        .map(|(id, agg)| {
            let score = if agg.covered == 0 {
                0.0
            } else {
                agg.passed as f64 / agg.covered as f64
            };
            let status = if agg.covered == 0 {
                "untested"
            } else if (score - 1.0).abs() < f64::EPSILON {
                "done"
            } else {
                "failing"
            };

            RequirementResult {
                id,
                weight: agg.weight,
                covered_cases: agg.covered,
                passing_cases: agg.passed,
                score,
                status: status.to_string(),
            }
        })
        .collect::<Vec<_>>();

    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn map_requirements(spec: &RequirementSpec, features: &[String]) -> BTreeSet<String> {
    let mut mapped = BTreeSet::new();
    for requirement in spec.mapped_requirements(features) {
        mapped.insert(requirement.to_string());
    }
    mapped
}

fn compute_summary(requirements: &[RequirementResult], cases: &[CaseResult]) -> ReportSummary {
    let total_cases = cases.len();
    let passed_cases = cases.iter().filter(|case| case.passed).count();
    let case_parity = if total_cases == 0 {
        0.0
    } else {
        passed_cases as f64 / total_cases as f64
    };

    let total_weight: f64 = requirements.iter().map(|item| item.weight).sum();
    let weighted_sum: f64 = requirements
        .iter()
        .map(|item| item.score * item.weight)
        .sum();
    let weighted_requirement_parity = if total_weight == 0.0 {
        0.0
    } else {
        weighted_sum / total_weight
    };

    let completed = requirements
        .iter()
        .filter(|item| item.status == "done")
        .count();
    let requirement_completion = if requirements.is_empty() {
        0.0
    } else {
        completed as f64 / requirements.len() as f64
    };

    let untested_requirements = requirements
        .iter()
        .filter(|item| item.status == "untested")
        .count();

    ReportSummary {
        total_cases,
        passed_cases,
        case_parity,
        weighted_requirement_parity,
        requirement_completion,
        untested_requirements,
    }
}

fn write_report_artifacts(config: &ParityConfig, report: &ParityReport) -> Result<()> {
    std::fs::create_dir_all(&config.output_dir)
        .with_context(|| format!("failed creating output dir {}", config.output_dir.display()))?;

    let json_path = config.output_dir.join("parity-report.json");
    let md_path = config.output_dir.join("parity-report.md");

    let json = serde_json::to_string_pretty(report).context("failed serializing parity report")?;
    std::fs::write(&json_path, json)
        .with_context(|| format!("failed writing {}", json_path.display()))?;

    let markdown = render_markdown(report);
    std::fs::write(&md_path, markdown)
        .with_context(|| format!("failed writing {}", md_path.display()))?;

    info!(json = %json_path.display(), markdown = %md_path.display(), "wrote parity report artifacts");
    Ok(())
}

fn render_markdown(report: &ParityReport) -> String {
    let mut out = String::new();
    out.push_str("# Iron Pony Parity Report\n\n");
    out.push_str(&format!(
        "- Generated epoch: `{}`\n",
        report.generated_epoch_secs
    ));
    out.push_str(&format!("- Cases: `{}`\n", report.summary.total_cases));
    out.push_str(&format!("- Passed: `{}`\n", report.summary.passed_cases));
    out.push_str(&format!(
        "- Case parity: `{:.2}%`\n",
        report.summary.case_parity * 100.0
    ));
    out.push_str(&format!(
        "- Weighted requirement parity: `{:.2}%`\n",
        report.summary.weighted_requirement_parity * 100.0
    ));
    out.push_str(&format!(
        "- Requirement completion: `{:.2}%`\n",
        report.summary.requirement_completion * 100.0
    ));
    out.push_str(&format!(
        "- Untested requirements: `{}`\n\n",
        report.summary.untested_requirements
    ));

    out.push_str("## Requirements\n\n");
    out.push_str("| Requirement | Status | Score | Covered | Passing | Weight |\n");
    out.push_str("|---|---|---:|---:|---:|---:|\n");
    for req in &report.requirements {
        out.push_str(&format!(
            "| {} | {} | {:.2}% | {} | {} | {:.2} |\n",
            req.id,
            req.status,
            req.score * 100.0,
            req.covered_cases,
            req.passing_cases,
            req.weight
        ));
    }

    out.push_str("\n## Cases\n\n");
    out.push_str("| Case | Passed | Exit | Stdout | Stderr |\n");
    out.push_str("|---|---|---|---|---|\n");
    for case in &report.cases {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            case.id, case.passed, case.exit_match, case.stdout_match, case.stderr_match
        ));
    }

    out
}

fn substitute_vars(input: &str, temp: &Path, workspace: &Path) -> String {
    input
        .replace("{temp}", &temp.to_string_lossy())
        .replace("{workspace}", &workspace.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_program_name_from_argv() {
        let normalized = normalize_argv(vec!["ponysay".to_string(), "--help".to_string()]);
        assert_eq!(normalized, vec!["--help"]);
    }

    #[test]
    fn mismatch_reports_length() {
        let detail = first_mismatch(b"abc", b"ab", "a", "b");
        assert!(detail.contains("length mismatch"));
    }
}
