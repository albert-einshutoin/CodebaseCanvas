//! Assemble the existing v0.1 recognizers without owning CLI output or filesystem writes.
use crate::{
    CallAnalysis, CallMode, CallScope, GraphBuilder, GraphMetadata, SystemGraph, calls,
    discovery::RepositoryRoot, nestjs_di, nestjs_modules, nestjs_roles, nestjs_routes, prisma,
    resolver::ImportResolver,
};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Analysis {
    pub graph: SystemGraph,
    /// Read TypeScript/TSX source files, including files with parse diagnostics.
    pub typescript_sources: usize,
    /// Selected and read Prisma schema files, either zero or one.
    pub prisma_schemas: usize,
}

pub fn analyze(root: &RepositoryRoot, analyzed_at: &str) -> Result<Analysis, String> {
    let resolver = ImportResolver::analyze(root)?;
    let typescript_sources = resolver.sources().count();
    let root_name = root
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && !name.chars().any(char::is_control))
        .unwrap_or("repository")
        .to_owned();
    let mut builder = GraphBuilder::new(GraphMetadata {
        analyzer_version: env!("CARGO_PKG_VERSION").into(),
        analyzed_at: analyzed_at.into(),
        root_name: Some(root_name),
        call_analysis: CallAnalysis {
            scope: CallScope::ParsedNamedClassMethods,
            mode: CallMode::SameClassOnly,
            examined_calls: 0,
            emitted_calls: 0,
            skipped_calls: 0,
        },
    });
    for (file, _) in resolver.sources() {
        nestjs_roles::extract_file(file, &resolver, &mut builder)?;
    }
    nestjs_modules::analyze(&resolver, &builder)?.apply(&mut builder)?;
    nestjs_routes::analyze(&resolver, &builder)?.apply(&mut builder)?;
    nestjs_di::analyze(&resolver, &builder)?.apply(&mut builder)?;
    calls::analyze(&resolver, &builder)?.apply(&mut builder)?;
    resolver.apply_imports(&mut builder)?;
    let prisma = prisma::analyze(root)?;
    let prisma_schemas = usize::from(prisma.selected_schema.is_some());
    prisma.apply(&mut builder, resolver.diagnostics())?;
    Ok(Analysis {
        graph: builder.finish()?,
        typescript_sources,
        prisma_schemas,
    })
}

pub fn utc_now() -> Result<String, String> {
    utc_timestamp(SystemTime::now())
}

fn utc_timestamp(time: SystemTime) -> Result<String, String> {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system time precedes the Unix epoch")?
        .as_secs();
    let days = i64::try_from(seconds / 86_400).map_err(|_| "system time is out of range")?;
    let z = days
        .checked_add(719_468)
        .ok_or("system time is out of range")?;
    let era = z / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_phase = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_phase + 2) / 5 + 1;
    let month = month_phase + if month_phase < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    if !(0..=9999).contains(&year) {
        return Err("system time is out of range".into());
    }
    let clock = seconds % 86_400;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        clock / 3_600,
        clock / 60 % 60,
        clock % 60
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_timestamp_handles_epoch_leap_day_and_year_boundary() {
        for (seconds, expected) in [
            (0, "1970-01-01T00:00:00Z"),
            (1_582_934_399, "2020-02-28T23:59:59Z"),
            (1_582_934_400, "2020-02-29T00:00:00Z"),
            (1_735_689_599, "2024-12-31T23:59:59Z"),
        ] {
            assert_eq!(
                utc_timestamp(UNIX_EPOCH + std::time::Duration::from_secs(seconds)).unwrap(),
                expected
            );
        }
        assert!(utc_timestamp(UNIX_EPOCH - std::time::Duration::from_secs(1)).is_err());
    }
}
