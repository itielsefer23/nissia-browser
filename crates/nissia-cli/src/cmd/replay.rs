use anyhow::Result;

pub async fn run(port: u16, name: &str, speed: f64, fmt: &str, dry_run: bool) -> Result<()> {
    if dry_run {
        let workflow = nissia_core::record::Workflow::load(name)
            .map_err(|e| anyhow::anyhow!("Failed to load workflow '{name}': {e}"))?;
        if fmt == "json" {
            println!(
                "{}",
                serde_json::json!({
                    "status": "dry_run",
                    "action": "replay",
                    "name": name,
                    "steps": workflow.steps.len(),
                })
            );
        } else {
            println!("[dry-run] replay {name} ({} steps)", workflow.steps.len());
        }
        return Ok(());
    }

    let transport = nissia_cdp::connect(port).await?;

    transport.send(&nissia_cdp::commands::PageEnable {}).await?;

    let result = nissia_core::record::replay::execute(&transport, name, speed).await?;

    if fmt == "json" {
        println!(
            "{}",
            serde_json::json!({
                "status": "ok",
                "action": "replay",
                "completed": result.completed,
                "total": result.total_steps,
                "warnings": result.warnings,
            })
        );
    } else {
        println!(
            "Replay complete: {}/{} steps",
            result.completed, result.total_steps
        );
        // Show all snap outputs
        for snap in &result.snaps {
            println!("---\n{}", snap.output);
        }
        // Show all read outputs
        for read in &result.reads {
            println!("---\n{}", read.output);
        }
        // Show all eval outputs
        for eval in &result.evals {
            match eval {
                serde_json::Value::String(s) => println!("---\n{s}"),
                serde_json::Value::Null => {}
                other => println!(
                    "---\n{}",
                    serde_json::to_string_pretty(other).unwrap_or_default()
                ),
            }
        }
        for warning in &result.warnings {
            eprintln!("warning: {warning}");
        }
        if let Some(step) = result.failed_step {
            eprintln!("Failed at step {step}");
        }
    }

    Ok(())
}
