# blrs-cli

#### `blrs-cli` is a command-line tool designed to simplify the management of multiple Blender builds. It allows you to easily download, install, and switch between various versions, making it ideal for artists who work with different project requirements or addon compatibilities.

This was built as an alternative to the popular Blender-Launcher (and its successor Blender-Launcher-V2) which fulfills
the same basic purpose without too many extra features.


### Key Features:

- **Download from multiple sources**: Access Blender builds directly from official repositories and community-maintained archives.
    - Currently, the only supported API is the official builder JSON api. More coming in the future for BForArtists and others!

- Version management: Search for specific versions by name, release date, or other criteria.
- **Simplified installation**: Automatically extract and organize downloaded builds within a dedicated library directory.
- **Easy switching**: Seamlessly switch between installed Blender versions with a single command.
- **Trashing & Removal**: Safely discard unwanted builds without manual intervention.

### Benefits

- **Enhanced organization**: Keep your Blender builds neatly organized and easily accessible.
- **Simplified workflow**: Streamline your development process by effortlessly switching between versions for different projects.
- **Scriptable**: Use shell and batch scripts as shortcuts to different versions.


### Target Audience

- **3D Artists**: Manage multiple Blender versions for diverse project needs.
- **Developers**: Work with specific Blender releases for addon testing or compatibility checks.
- Anyone who uses Blender regularly and values a streamlined workflow.

## Installation
If you have cargo installed:
```sh
cargo install blrs-cli --git https://github.com/zeptofine/blrs-cli
# the command `blrs` should now be available.
```

## TODO

- [ ] Extraction of .dmg files for macOS


## Contributing

blrs-cli is open-source and welcomes contributions. If you have ideas, bug fixes, or enhancements, please feel free to contribute to the project on GitHub. I (zeptofine) am still relatively new to Rust, so feedback is appreciated!




License
---
blrs-cli is licensed under the Apache 2.0 License. A full copy of the license is provided in [LICENSE](LICENSE).

```

   blrs-cli

   Copyright 2024 Erin MacDonald

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
```
