use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use async_trait::async_trait;
use aws_sdk_s3::{primitives::ByteStream, Client};
use error_stack::{Result, ResultExt};

use crate::SinkParquetError;

#[async_trait]
pub trait ParquetWriter {
    async fn write_parquet(&mut self, path: PathBuf, data: &[u8]) -> Result<(), SinkParquetError>;
}

pub struct FileParquetWriter;

#[async_trait]
impl ParquetWriter for FileParquetWriter {
    async fn write_parquet(&mut self, path: PathBuf, data: &[u8]) -> Result<(), SinkParquetError> {
        let path = if path.starts_with("file://") {
            // Safe to unwrap because we know the path starts with "file://"
            path.strip_prefix("file://").unwrap()
        } else {
            &path
        };

        let output_dir = path
            .parent()
            .ok_or(SinkParquetError)
            .attach_printable(format!("cannot get parent directory of `{path:?}`"))?;

        fs::create_dir_all(output_dir)
            .change_context(SinkParquetError)
            .attach_printable(format!(
                "failed to create output directory `{output_dir:?}`"
            ))?;

        let mut file = File::create(path)
            .change_context(SinkParquetError)
            .attach_printable(format!("failed to create parquet file at `{path:?}`"))?;

        file.write_all(data)
            .change_context(SinkParquetError)
            .attach_printable(format!("failed to write parquet to file at `{path:?}`"))?;

        Ok(())
    }
}

pub struct S3ParquetWriter {
    pub client: Client,
}

#[async_trait]
impl ParquetWriter for S3ParquetWriter {
    async fn write_parquet(&mut self, path: PathBuf, data: &[u8]) -> Result<(), SinkParquetError> {
        let path = path
            .as_os_str()
            .to_str()
            .ok_or(SinkParquetError)
            .attach_printable(format!("cannot convert path `{path:?}` to string"))?;

        let mut path_parts = path
            .strip_prefix("s3://")
            .ok_or(SinkParquetError)
            .attach_printable(format!("provided path is not an s3 URL `{path:?}`"))?
            .split('/');

        let bucket_name = path_parts
            .next()
            .and_then(|bucket_name| {
                if bucket_name.is_empty() {
                    None
                } else {
                    Some(bucket_name)
                }
            })
            .ok_or(SinkParquetError)
            .attach_printable(format!("cannot get the bucket name from `{path:?}`"))?;

        let key = path_parts.collect::<Vec<&str>>().join("/");
        let body = ByteStream::from(data.to_vec());

        let result = self
            .client
            .put_object()
            .bucket(bucket_name)
            .key(key)
            .body(body)
            .send()
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(err) => Err(SinkParquetError)
                .attach_printable(format!("failed to write parquet to s3 at `{path:?}`"))
                // For some reason, we need to attach the error to the report,
                // otherwise the error is not printed.
                .attach_printable(format!("error: {err:?}")),
        }
    }
}
