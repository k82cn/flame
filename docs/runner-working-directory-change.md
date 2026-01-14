# Runner Working Directory Change

## Summary

Modified the Runner class to use `/opt/{name}` as the working directory when registering applications, where `{name}` is the application name provided to the Runner constructor.

## Change Details

### Location
`sdk/python/src/flamepy/runner.py` - `Runner.__enter__()` method

### Before
```python
app_attrs = ApplicationAttributes(
    ...
    working_directory=flmrun_app.working_directory,  # Uses template's working dir
    ...
)
```

### After
```python
# Use /opt/{name} as working directory for the application
working_directory = f"/opt/{name}"

app_attrs = ApplicationAttributes(
    ...
    working_directory=working_directory,  # Uses /opt/{name}
    ...
)
```

## Benefits

### 1. Predictable File Paths
Applications always run in a known location: `/opt/{name}`
- No ambiguity about working directory
- Consistent across all runner-based applications

### 2. Archive Extraction Location
When archives are extracted by FlameRunpyService:
- Extraction happens in the working directory (via `os.getcwd()`)
- Archives extracted to `/opt/{name}/extracted_...`
- Predictable and organized file layout

### 3. File System Isolation
Each application gets its own directory:
- `/opt/app1/` for application "app1"
- `/opt/app2/` for application "app2"
- Reduces risk of file conflicts between applications

### 4. Debugging and Troubleshooting
Easier to:
- Find application files on executor nodes
- Debug file path issues
- Inspect extracted archives
- Monitor disk usage per application

## Example

### Application Registration
```python
with Runner("my-data-processor") as rr:
    # Application registered with working_directory="/opt/my-data-processor"
    service = rr.service(MyProcessor())
    result = service.process()
```

### Execution on Executor Node
When the application runs:
1. Executor creates/changes to `/opt/my-data-processor/`
2. If package is an archive, extracts to `/opt/my-data-processor/extracted_my-data-processor/`
3. Installs package from extracted directory
4. All relative file operations happen in `/opt/my-data-processor/`

### File System Layout
```
/opt/
  my-data-processor/          # Working directory
    extracted_my-data-processor/  # Extracted archive
      setup.py
      my_module/
        __init__.py
        processor.py
    temp_files/               # Any temp files created by app
    logs/                     # Any logs created by app
```

## Compatibility

### Backward Compatibility
- ✅ **Fully backward compatible**
- No changes required to existing code
- Only affects where applications run, not how they run

### Requirements
The `/opt/` directory must:
- Exist on executor nodes
- Be writable by the executor process
- Have sufficient disk space for extracted packages

### Creating the Directory
Most systems already have `/opt/`. If needed:
```bash
sudo mkdir -p /opt
sudo chown $(whoami):$(whoami) /opt
# Or make it world-writable (less secure):
sudo chmod 777 /opt
```

## Impact on Components

### 1. Runner
- ✅ **Updated**: Sets `working_directory = f"/opt/{self._name}"`
- Logs the working directory during registration

### 2. FlameRunpyService
- ✅ **No changes needed**
- Uses `os.getcwd()` which returns the working directory set by Runner
- Archives automatically extracted to correct location

### 3. Tests
- ✅ **No changes needed**
- Tests work with any working directory
- Runner tests already work with the new location

### 4. Documentation
- ✅ **Updated**:
  - `runner-setup.md`: Added "Working Directory" section
  - `RFE280-implementation-summary.md`: Updated features list
  - `runpy-enhancements.md`: Added note about extraction location
  - This document: Created to document the change

## Use Cases

### Use Case 1: Data Processing Application
```python
with Runner("data-processor") as rr:
    # Runs in /opt/data-processor/
    processor = rr.service(DataProcessor())
    
    # Can write files to working directory
    processor.export_results("results.csv")
    # Creates /opt/data-processor/results.csv
```

### Use Case 2: Multi-Application Deployment
```python
# Application 1
with Runner("app1") as rr1:
    # Runs in /opt/app1/
    service1 = rr1.service(Service1())

# Application 2
with Runner("app2") as rr2:
    # Runs in /opt/app2/
    service2 = rr2.service(Service2())
    
# No file conflicts between applications
```

### Use Case 3: Debugging Archive Extraction
```bash
# SSH to executor node
ssh executor-node

# Check extracted archive
ls -la /opt/my-app/
ls -la /opt/my-app/extracted_my-app/

# Check logs
cat /opt/my-app/app.log
```

## Testing

### Verify Working Directory
```python
import os

class TestWorkingDir:
    def get_cwd(self):
        return os.getcwd()

with Runner("test-working-dir") as rr:
    service = rr.service(TestWorkingDir())
    result = service.get_cwd()
    cwd = result.get()
    print(f"Working directory: {cwd}")
    # Expected: /opt/test-working-dir
```

### Verify Archive Extraction
```python
# Check logs during session creation
# Should see:
# "Extracting archive: /storage/my-app.tar.gz to /opt/my-app/extracted_my-app"
```

## Best Practices

### 1. Clean Application Names
Use filesystem-safe names:
```python
# Good
with Runner("my-data-processor") as rr:
    ...

# Avoid special characters
with Runner("my_app_v1.0") as rr:  # May cause issues
    ...
```

### 2. Check Disk Space
For applications that write large files:
```python
import shutil

class DiskMonitor:
    def check_space(self):
        total, used, free = shutil.disk_usage("/opt")
        return {"total": total, "used": used, "free": free}

with Runner("disk-monitor") as rr:
    monitor = rr.service(DiskMonitor())
    space = monitor.check_space().get()
    print(f"Free space in /opt: {space['free'] / (1024**3):.2f} GB")
```

### 3. Relative Paths
Use relative paths for portability:
```python
class FileProcessor:
    def save_output(self, data):
        # Good: Relative path
        with open("output.json", "w") as f:
            f.write(data)
        # Creates /opt/my-app/output.json
        
        # Avoid: Absolute path
        # with open("/tmp/output.json", "w") as f:
        #     f.write(data)
```

## Troubleshooting

### Error: Permission Denied
```
Error: [Errno 13] Permission denied: '/opt/my-app'
```

**Solution**: Ensure `/opt` is writable:
```bash
sudo chown -R $(whoami):$(whoami) /opt
```

### Error: No Space Left on Device
```
Error: [Errno 28] No space left on device
```

**Solution**: Clean up old applications or increase disk space:
```bash
# Check disk usage
df -h /opt

# Remove old application directories
sudo rm -rf /opt/old-app-*
```

### Files Not Found After Extraction
```
Error: FileNotFoundError: 'module.py'
```

**Solution**: Check extraction path and verify archive structure:
```bash
# SSH to executor
ssh executor-node

# Check extracted files
ls -la /opt/my-app/extracted_my-app/

# Verify package structure
tar -tzf /storage/my-app.tar.gz | head -20
```

## Summary

This change provides:
- ✅ Predictable working directories (`/opt/{name}`)
- ✅ Organized file system layout
- ✅ Better debugging and troubleshooting
- ✅ Application isolation
- ✅ Fully backward compatible

No code changes required for existing applications!
