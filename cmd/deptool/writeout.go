package main

import (
	"bytes"
	"fmt"
	"os"
	"path"
	"strings"

	modfile "golang.org/x/mod/modfile"
)

func writeUpdates(dir string, deps map[string]analyzedProjectDependency, inplace bool) {
	writeUpdatesGoMod(dir, deps, inplace)
	writeUpdatesCargoToml(dir, deps, inplace)
}

func writeUpdatesGoMod(dir string, deps map[string]analyzedProjectDependency, inplace bool) {
	fileName := path.Join(dir, goModFile)

	modFileBytes, err := os.ReadFile(fileName)
	if err != nil {
		fmt.Printf("Unable to read go.mod file : %v\n", err)
		exitErr()
	}

	modFile, err := modfile.Parse("", modFileBytes, nil)
	if err != nil {
		fmt.Printf("Unable to read go.mod file : %v\n", err)
		exitErr()
	}

	changed := false
	for _, analyzed := range deps {
		if analyzed.class != depClassMod {
			continue
		}
		if analyzed.latestBranchCommit == analyzed.githubCommit {
			continue
		}
		// find if we have entry in the mod file.
		for _, req := range modFile.Require {
			if req.Mod.Path != analyzed.githubPath {
				continue
			}
			// this entry needs to be updated.
			splittedVersion := strings.Split(req.Mod.Version, "-")
			splittedVersion[2] = analyzed.latestBranchCommit[:len(splittedVersion[2])]
			splittedVersion[1] = fmt.Sprintf("%04d%02d%02d%02d%02d%02d",
				analyzed.latestBranchCommitTime.Year(),
				analyzed.latestBranchCommitTime.Month(),
				analyzed.latestBranchCommitTime.Day(),
				analyzed.latestBranchCommitTime.Hour(),
				analyzed.latestBranchCommitTime.Minute(),
				analyzed.latestBranchCommitTime.Second())
			newVer := fmt.Sprintf("%s-%s-%s", splittedVersion[0], splittedVersion[1], splittedVersion[2])
			curPath := req.Mod.Path
			err = modFile.DropRequire(req.Mod.Path)
			if err != nil {
				fmt.Printf("Unable to drop requirement : %v\n", err)
				exitErr()
			}
			err = modFile.AddRequire(curPath, newVer)
			if err != nil {
				fmt.Printf("Unable to add requirement : %v\n", err)
				exitErr()
			}
			changed = true
		}
	}

	if !changed {
		return
	}

	outputBytes, err := modFile.Format()
	if err != nil {
		fmt.Printf("Unable to format mod file : %v\n", err)
		exitErr()
	}
	if !inplace {
		fileName += ".proposed"
	}
	err = os.WriteFile(fileName, outputBytes, 0200)
	if err != nil {
		fmt.Printf("Unable to write %s file : %v\n", fileName, err)
		exitErr()
	}
	err = os.Chmod(fileName, 0644)
	if err != nil {
		fmt.Printf("Unable to chmod %s file : %v\n", fileName, err)
		exitErr()
	}
}

func writeUpdatesCargoToml(dir string, deps map[string]analyzedProjectDependency, inplace bool) {
	fileName := path.Join(dir, cargoTomlFile)

	modFileBytes, err := os.ReadFile(fileName)
	if err != nil {
		fmt.Printf("Unable to read go.mod file : %v\n", err)
		exitErr()
	}

	changed := false
	for _, analyzed := range deps {
		if analyzed.class != depClassCargo {
			continue
		}
		if analyzed.latestBranchCommit == analyzed.githubCommit {
			continue
		}
		newCommit := analyzed.latestBranchCommit[:len(analyzed.githubCommit)]
		// we want to replace every instance of analyzed.githubCommit with newCommit
		modFileBytes = bytes.ReplaceAll(modFileBytes, []byte(analyzed.githubCommit), []byte(newCommit))

		// set the changed flag
		changed = true
	}

	if !changed {
		return
	}
	if !inplace {
		fileName = fileName + ".proposed"
	}
	err = os.WriteFile(fileName, modFileBytes, 0200)
	if err != nil {
		fmt.Printf("Unable to write %s file : %v\n", fileName, err)
		exitErr()
	}
	err = os.Chmod(fileName, 0644)
	if err != nil {
		fmt.Printf("Unable to chmod %s file : %v\n", fileName, err)
		exitErr()
	}
}
