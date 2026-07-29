use secret_resolvers::{FileResolver, SecretResolver, ResolveRequest, ExposeSecret};
use tempfile::NamedTempFile;
use std::{collections::HashMap, io::prelude::Write};

fn setup_tmp_file(plaintext: &str, newline: bool)->(NamedTempFile, String){
    let mut datafile = tempfile::NamedTempFile::new().unwrap();
    let _ = datafile.write(plaintext.as_bytes()).unwrap();
    if newline {
        let _ = datafile.write(&[b'\n']).unwrap();
    }
    let path = datafile.path().to_string_lossy().to_string();
    (datafile, path)
}


#[tokio::test]
async fn test_file_resolves_existing_file() {
    const CONTENT: &str = "hello-world";
    let (_datafile, filename) = setup_tmp_file(CONTENT, false);
    let resolver = FileResolver::new(HashMap::new()).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([("ref".to_string(), toml::Value::String(filename))]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value.expose_secret(), CONTENT);
    assert_eq!(result.ttl, None);

    let (_datafile, filename) = setup_tmp_file(CONTENT, true);
    let request = ResolveRequest {
        params: HashMap::from([("ref".to_string(), toml::Value::String(filename))]),
    };
    let result = resolver.resolve(&request).await.unwrap();
    assert_eq!(result.value.expose_secret(), CONTENT);
    assert_eq!(result.ttl, None);
}

#[tokio::test]
async fn test_file_errors_on_missing_file() {
    let resolver = FileResolver::new(HashMap::new()).unwrap();
    let request = ResolveRequest {
        params: HashMap::from([("ref".to_string(), toml::Value::String("this file does not exist or the environment is broken".to_string()))]),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_file_errors_on_missing_ref_param() {
    let resolver = FileResolver::new(HashMap::new()).unwrap();
    let request = ResolveRequest {
        params: HashMap::new(),
    };
    let result = resolver.resolve(&request).await;
    assert!(result.is_err());
}
