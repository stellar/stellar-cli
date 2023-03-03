package main

import (
	"fmt"
	"os"
	"path"
	"sort"
	"strings"

	toml "github.com/pelletier/go-toml"
	modfile "golang.org/x/mod/modfile"
)

const cargoTomlFile = "Cargo.toml"
const goModFile = "go.mod"

type depClass int

const (
	depClassCargo depClass = iota
	depClassMod
)

type projectDependencies struct {
	dependencies    []*projectDependency
	dependencyNames map[string]*projectDependency
}

type projectDependency struct {
	class        depClass
	githubPath   string
	githubCommit string
	direct       bool
	version      string
	name         string
}

type cargoDependencyToml struct {
	Git     string `toml:"git"`
	Rev     string `toml:"rev"`
	Version string `toml:"version"`
}

type workspaceDepenenciesToml struct {
	Dependencies map[string]cargoDependencyToml `toml:"dependencies"`
}

type patchCratesIOToml struct {
	CratesIO map[string]cargoDependencyToml `toml:"crates-io"`
}

type cargoToml struct {
	Workspace workspaceDepenenciesToml // this is the workspace.dependencies entry; the toml decoder breaks it into workspace and depenencies
	Patch     patchCratesIOToml        // this is the patch.crates-io entry
}

func scanProject(dir string) *projectDependencies {
	dependencies := &projectDependencies{
		dependencyNames: make(map[string]*projectDependency),
	}

	loadParseCargoToml(dir, dependencies)
	loadParseGoMod(dir, dependencies)

	return dependencies
}

func loadParseCargoToml(dir string, dependencies *projectDependencies) {
	cargoFileBytes, err := os.ReadFile(path.Join(dir, cargoTomlFile))
	if err != nil {
		fmt.Printf("Unable to read Cargo.toml file : %v\n", err)
		exitErr()
	}

	var parsedCargo cargoToml
	err = toml.Unmarshal(cargoFileBytes, &parsedCargo)
	if err != nil {
		fmt.Printf("Unable to parse Cargo.toml file : %v\n", err)
		exitErr()
	}
	addTomlDependencies(dependencies, parsedCargo.Patch.CratesIO, false)
	addTomlDependencies(dependencies, parsedCargo.Workspace.Dependencies, true)
}

func addTomlDependencies(dependencies *projectDependencies, tomlDeps map[string]cargoDependencyToml, direct bool) {
	names := make([]string, 0, len(tomlDeps))
	for name := range tomlDeps {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, pkgName := range names {
		crateGit := tomlDeps[pkgName]
		if crateGit.Git == "" {
			continue
		}

		current := &projectDependency{
			class:        depClassCargo,
			githubPath:   crateGit.Git,
			githubCommit: crateGit.Rev,
			version:      crateGit.Version,
			direct:       direct,
			name:         pkgName,
		}
		if existing, has := dependencies.dependencyNames[pkgName]; has && (existing.githubCommit != current.githubCommit || existing.githubPath != current.githubPath) {
			fmt.Printf("Conflicting entries in Cargo.toml file :\n%v\nvs.\n%v\n", existing, current)
			exitErr()
		}
		if current.githubPath == "" {
			continue
		}
		dependencies.dependencyNames[pkgName] = current
		dependencies.dependencies = append(dependencies.dependencies, current)
	}
}

func loadParseGoMod(dir string, dependencies *projectDependencies) {
	fileName := path.Join(dir, goModFile)

	cargoFileBytes, err := os.ReadFile(fileName)
	if err != nil {
		fmt.Printf("Unable to read go.mod file : %v\n", err)
		exitErr()
	}

	modFile, err := modfile.Parse("", cargoFileBytes, nil)
	if err != nil {
		fmt.Printf("Unable to read go.mod file : %v\n", err)
		exitErr()
	}
	// scan all the stellar related required modules.
	for _, require := range modFile.Require {
		if !strings.Contains(require.Mod.Path, "github.com/stellar") || require.Indirect {
			continue
		}
		splittedVersion := strings.Split(require.Mod.Version, "-")
		if len(splittedVersion) != 3 {
			continue
		}

		pathComp := strings.Split(require.Mod.Path, "/")
		pkgName := pathComp[len(pathComp)-1]

		current := &projectDependency{
			class:        depClassMod,
			githubPath:   require.Mod.Path,
			githubCommit: splittedVersion[2],
			direct:       true,
			name:         pkgName,
		}

		if existing, has := dependencies.dependencyNames[pkgName]; has && (existing.githubCommit != current.githubCommit || existing.githubPath != current.githubPath) {
			fmt.Printf("Conflicting entries in go.mod file :\n%v\nvs.\n%v\n", existing, current)
			exitErr()
		}
		dependencies.dependencyNames[pkgName] = current
		dependencies.dependencies = append(dependencies.dependencies, current)
	}
}
