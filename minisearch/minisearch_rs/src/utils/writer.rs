use std::{
    ffi::OsString,
    fs::{self, File},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub struct DocumentsWriter {
    pub dir: OsString,
    cur_segment: OsString,
}

impl DocumentsWriter {
    pub fn new(dir: OsString) -> Self {
        let cur_segment = match fs::exists(&dir) {
            Ok(_) => {
                let mut segment: u64 = 0;
                let mut segment_path = None;
                for e in fs::read_dir(&dir).unwrap() {
                    let path = e.unwrap().path();
                    if !path.is_dir() {
                        continue;
                    }

                    let name = match path
                        .file_name()
                        .unwrap()
                        .to_os_string()
                        .to_str()
                        .unwrap()
                        .parse::<u64>()
                    {
                        Ok(val) => val,
                        Err(_) => 0,
                    };

                    if name > segment {
                        segment = name;
                        segment_path = Some(path);
                    }
                }

                match segment_path {
                    Some(path) => path.into_os_string(),
                    None => Self::create_segment(&dir),
                }
            }
            Err(_) => {
                fs::create_dir_all(&dir);
                Self::create_segment(&dir)
            }
        };

        Self {
            dir: dir,
            cur_segment: OsString::new(),
        }
    }

    fn create_segment(dir: &OsString) -> OsString {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let segment = Path::new(dir).join(ts);
        fs::create_dir(&segment);

        for f in ["segment", "meta", "del"] {
            File::create(segment.join(f));
        }

        segment.as_os_str().to_os_string()
    }
}
