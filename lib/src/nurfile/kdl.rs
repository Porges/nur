use std::str::FromStr;

use kdl::KdlDocument;
use miette::IntoDiagnostic;

use crate::{version::Version, Error};

impl TryFrom<KdlDocument> for crate::nurfile::NurFile {
    type Error = Error;
    fn try_from(value: KdlDocument) -> Result<Self, Error> {
        let version_str =
            || -> Option<&str> { value.get("version")?.entries().first()?.value().as_string() }()
                .ok_or(Error::MissingVersion)?;

        let _version = Version::from_str(version_str).map_err(Error::InvalidVersion)?;

        todo!();
        /*
        Ok(crate::nurfile::NurFile {
            version,
            options: todo!(),
            lets: todo!(),
            tasks: todo!(),
            env: todo!(),
        })
        */
    }
}

pub fn parse(_path: &std::path::Path, input: &str) -> miette::Result<crate::nurfile::NurFile> {
    let doc: KdlDocument = input.parse().into_diagnostic()?; // TODO: remove into_diagnostic when on same miette version

    //let displayable_filename = path.display().to_string();
    //let nf: NurKdl = kdl::parse(&displayable_filename, input)?;

    Ok(doc.try_into()?)
}
