# rmake

rmake builds existing Makefiles on Linux with a Rust engine. Keep your
Makefile and system `make`, then run `rmake` alongside it with the same
targets, parallel options, and recursive `$(MAKE)` calls.

The goal is to make Linux builds faster while matching GNU Make 4.4.1.
CI compares exact output and file effects and builds pinned projects such as
zlib and Lua. One recorded workload with 5,000 command-line variables took
about a third of GNU Make's time. See the
[benchmark guide](docs/benchmarks/README.md) to measure your own workload.
Compatibility is still in development. Compare clean and incremental builds
with both tools before switching a build you depend on.

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

Create a small project to see when rmake runs a recipe.

```sh
mkdir rmake-example
cd rmake-example
printf '%s\n' 'Hello from rmake' > message.txt
```

Save this example as `Makefile`. Start the recipe line with a tab.

```makefile
greeting.txt: message.txt
	cp "$<" "$@"
```

Run the build twice, then edit the input and rebuild.

```sh
rmake
rmake
sleep 1
printf '%s\n' 'Hello again' >> message.txt
rmake
```

The first run creates `greeting.txt`. The second skips the recipe because
the target is current. The pause gives the edit a newer timestamp. The last
run copies the changed input. In the recipe,
`$<` expands to the first prerequisite and `$@` expands to the target.
See [automatic variables](https://www.gnu.org/software/make/manual/make.html#Automatic-Variables)
in the GNU Make manual.

## Try rmake on an existing project

From a directory that contains your project's Makefile, run `rmake` for the
default goal or pass a goal by name. Use `-j4` to allow up to four jobs.

```sh
rmake -j4
```

Use [output synchronization](docs/output-sync.md) to keep parallel build logs
readable.

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
