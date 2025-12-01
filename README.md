# blrs-cli

#### `blrs-cli` is a command-line tool designed to simplify the management of multiple Blender builds. It allows you to easily download, install, and switch between various versions, making it ideal for artists who work with different project requirements or addon compatibilities, or developers who want to test their addons on various blender builds.

This was built as an alternative to the popular Blender-Launcher (and its successor Blender-Launcher-V2) which fulfills
the same basic purpose without too many extra features, and with scriptability in mind.

I'll be honest, this isn't as user friendly as BLV2, but that wasn't the goal. I just wanted a small way to house blender builds and update easily. It essentially fulfills my own usecase and not much else lol..


- **Access Blender builds directly from official repositories and community-maintained archives.**
    - Currently, the only supported API is the official builder JSON api. More coming in the future for BForArtists and others!
- **Search for specific versions by name, release date, or other criteria.**
- **Safely discard unwanted builds without manual intervention.**

## Installation
If you have cargo installed:
```sh
cargo install blrs-cli --git https://github.com/zeptofine/blrs-cli
# the command `blrs` should now be available.
```

## Usage

The program automatically generates a config file at `~/.config/blrs/config.toml` for linux, and `%LOCALAPPDATA%/zeptofine/blrs/config/config.toml` for Windows.

In there, you can customize the library path, repo cache placement, and individual repo's settings and sources.

This project uses a specific query syntax to identify and filter out builds:
```
[repo/]<major>.<minor>[.<patch>][-<branch>][[+ or #]<build_hash>][@<commit time>]
```

The major, minor, and patch numbers can be integers, or one of these:
- `^`: Match the largest/newest item
- `*`: Match any item
- `-`: Match the smallest/oldest item

The commit time HAS to be one of these. By default it is "*"

### The blrs command has multiple subcommands:

#### Fetch - Fetches the latest builds from the blender repositories. Does not download any builds.
```
Usage: blrs fetch [OPTIONS]

Options:
  -f, --force
          Ignore fetch timeouts. 
  -p, --parallel
          Runs fetching from repos in parallel using async features. Can trigger ratelimits if used recklessly
  -i, --ignore-errors
          If true, if an error occurs then it will continue trying to fetch the rest of the repos.
          The return code of the program reflects the very first error that occurs.
```

#### Pull: Downloads a build from the databases. You can download and install multiple builds at the same time.
```
Usage: blrs pull [OPTIONS] [QUERIES]...

Arguments:
  [QUERIES]...  The version matchers to find the correct build

Options:
  -a, --all-platforms  
```

#### Rm: Sends a build to the trash or deletes the build.
```
Usage: blrs rm [OPTIONS] [QUERIES]...

Arguments:
  [QUERIES]...  

Options:
  -n, --no-trash  Tries to fully delete a file, and does not send the file to the trash
```

#### Ls: lists all the builds in the database.
```
Usage: blrs ls [OPTIONS]

Options:
  -f, --format <FORMAT>
          Possible values:
          - tree:        A visual tree. Good for human interpretation, but not easily parsed
          - paths:       Shows filepaths of builds. Only shows installed
          - json:        single-line JSON format
          - pretty-json: Json but indented by 2 spaces to make it more human readable

      --sort-by <SORT_BY>
          [possible values: version, datetime]

  -i, --installed-only
          Filter out only builds that are installed

  -v, --variants
          Show individual variants for remote builds

  -a, --all-builds
          Shows all builds, even if they are not for your target os. Our filtering is not perfect. this may be necessary for you to find the proper build
```

#### Run: Runs a build.
##### Run subcommands:

File: open a specific file and assume the correct build, by using header information to detect what build the file was made in and checking if it exists in the repos.

`blrs run file <PATH>` 

Build: Launches a specific build of blender. You can pass extra arguments to the blender instance. 

`blrs run build [BUILD] [-- <ARGS>...]`

## Example

Fetching the repos, downloading the latest daily build, then launching it

```bash
blrs fetch # updates build list
blrs pull "daily/^.^.^" # or "4.5" as of writing this 
blrs run build "daily/^.^.^@^"
```

You should be able to tread `blrs run ...` as a replacement for running the underlying build directly. If you have troubles with this, please make an issue! This is my personal goto `blatest` alias:


```bash
env _NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia blrs run build daily/^.^.^@^ --
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
