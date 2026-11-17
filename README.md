# rmake

rmake is a Rust implementation of GNU Make for Linux. It reads existing
Makefiles and builds their targets using the rules, variables, and recipes you
already have. Run `rmake` alongside `make` to evaluate it on your project.

Use rmake to try a new build engine with familiar commands, including parallel
builds and recursive `$(MAKE)` calls. The project aims to make Linux builds
faster and compares its results with GNU Make 4.4.1 in CI. Compatibility is
still in development. If you depend on a build, compare both tools on a clean
build and again after changing an input.

## Build rmake

You need x86-64 Linux with glibc, Git, GNU Make, Python 3.11 or later, a C
toolchain, and network access to download the pinned Rust toolchain and Cargo
dependencies.

```sh
git clone https://github.com/jeffdasilva/rmake.git
cd rmake
make bootstrap-rust
make release
./target/release/rmake --version
```

`make bootstrap-rust` installs Rust inside the checkout. To run `rmake` from any
directory in your current shell, install the executable to your user bin
directory and add it to `PATH`.

```sh
install -Dm755 target/release/rmake "$HOME/.local/bin/rmake"
export PATH="$HOME/.local/bin:$PATH"
```

## Run your first build

In an empty directory, create a file named `message.txt` with any text. Then
save this example as `Makefile`. Start the recipe line with a tab.

```makefile
greeting.txt: message.txt
	cp "$<" "$@"
```

Run `rmake` twice. The first run creates `greeting.txt`. The second sees that
the target is up to date and skips the recipe. Change `message.txt` and run
`rmake` again to rebuild it. Learn how targets, prerequisites, and recipes fit
together in the [GNU Make introduction](https://www.gnu.org/software/make/manual/html_node/Introduction.html).

## Try rmake on an existing project

From a directory that contains your project's Makefile, run `rmake` for the
default goal or pass a goal by name. Use `-j4` to allow up to four jobs.

```sh
rmake -j4
```

For rule syntax, recipe behavior, and options, start with the
[GNU Make manual](https://www.gnu.org/software/make/manual/html_node/index.html).
For tested projects, known differences, and platform support, see the
[rmake compatibility guide](docs/compatibility.md).

## Contribute to rmake

A Makefile that behaves differently under the two commands is a good place to
start. When [reporting a difference](https://github.com/jeffdasilva/rmake/issues),
include the smallest Makefile you can reproduce it with, the complete command,
the two versions, and what each command did.

From the rmake checkout, prepare the comparison tools and run the developer
checks.

```sh
make bootstrap
make check
```

Use `make help` for build, test, and benchmark commands. To understand how the
project checks compatibility or measures speed, see the
[testing guide](docs/testing.md) and [benchmark reports](docs/benchmarks/README.md).
The [architecture guide](docs/architecture.md) explains where to make changes.

## License

rmake is published under the [GNU General Public License, version 3 or later](LICENSE).
See [licensing and provenance](docs/licensing.md) for details.
