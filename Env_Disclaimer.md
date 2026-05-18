.cargo/config.toml currently hardcodes paths that exist on my machine:

'''
Visual Studio 2022 Community
MSVC 14.43.34808
Windows SDK 10.0.26100.0
'''

So it makes cargo test work in this checkout on this PC, but it may break for someone else if they have:

'''
Visual Studio Build Tools instead of Community
a different MSVC version
a different Windows SDK version
Visual Studio installed on another drive
Linux/macOS
'''

In general, the portable approach is:

'''
Run Cargo from a Developer PowerShell/Developer Command Prompt
or configure CI to install/setup MSVC before running cargo
'''

Repo-local hardcoded toolchain paths are useful as a local workaround, but usually should not be committed as project configuration. A better committed setup would be docs or scripts, for example scripts/dev-shell.ps1, that calls vcvars64.bat dynamically.