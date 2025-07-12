use std::time::SystemTime;

pub struct Process {
    pub pid: u32,
    pub name: Option<String>,
    pub exe: Option<String>,
    pub current_memory_usage: u64,
    pub max_memory_usage: u64,
    pub first_seen: SystemTime,
    pub last_seen: SystemTime,
}

impl Process {
    pub fn new(
        pid: u32,
        name: Option<String>,
        exe: Option<String>,
        memory_usage: u64,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            pid,
            name,
            exe,
            current_memory_usage: memory_usage,
            max_memory_usage: memory_usage,
            first_seen: now,
            last_seen: now,
        }
    }

    pub fn update(&mut self, name: Option<String>, exe: Option<String>, memory_usage: u64) {
        if let Some(new_name) = name {
            self.name = Some(new_name);
        }
        if let Some(new_exe) = exe {
            self.exe = Some(new_exe);
        }
        self.current_memory_usage = memory_usage;
        self.max_memory_usage = self.max_memory_usage.max(memory_usage);
        self.last_seen = SystemTime::now();
    }
}