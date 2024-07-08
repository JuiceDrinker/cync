use aws_sdk_s3 as s3;
use aws_smithy_async::future::pagination_stream::PaginationStream;
use aws_smithy_runtime_api::client::orchestrator::HttpResponse;
use s3::{
    error::SdkError,
    operation::{
        get_object::{GetObjectError, GetObjectOutput},
        list_objects_v2::{ListObjectsV2Error, ListObjectsV2Output},
        put_object::{PutObjectError, PutObjectOutput},
    },
};

pub struct S3Client {
    inner: s3::Client,
}

impl S3Client {
    pub fn new(inner: s3::Client) -> Self {
        Self { inner }
    }

    pub async fn put_object(
        &self,
        bucket_name: impl Into<String>,
        object_name: impl Into<String>,
        body: s3::primitives::ByteStream,
    ) -> Result<PutObjectOutput, SdkError<PutObjectError>> {
        self.inner
            .put_object()
            .bucket(bucket_name)
            .key(object_name)
            .body(body)
            .send()
            .await
    }

    pub async fn list_objects(
        &self,
        bucket_name: impl Into<String>,
    ) -> PaginationStream<Result<ListObjectsV2Output, SdkError<ListObjectsV2Error, HttpResponse>>>
    {
        self.inner
            .list_objects_v2()
            .bucket(bucket_name)
            .max_keys(10)
            .into_paginator()
            .send()
    }

    pub async fn get_object(
        &self,
        bucket_name: impl Into<String>,
        file_path: impl Into<String>,
    ) -> Result<GetObjectOutput, SdkError<GetObjectError, HttpResponse>> {
        self.inner
            .get_object()
            .bucket(bucket_name)
            .key(file_path)
            .send()
            .await
    }
}
