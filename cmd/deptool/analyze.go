package main

import (
	"errors"
	"fmt"
	"strings"
	"time"

	git "github.com/go-git/go-git/v5"

	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
	"github.com/go-git/go-git/v5/plumbing/storer"
	"github.com/go-git/go-git/v5/storage/memory"
)

type analyzedProjectDependency struct {
	projectDependency
	branchName             string
	fullCommitHash         string
	latestBranchCommit     string
	latestBranchCommitTime time.Time
	latestBranchVersion    string
	workspaceVersion       bool // is the version is defined per workspace or package ?
}

type analyzedDependencyFunc func(string, analyzedProjectDependency)

func analyze(dependencies *projectDependencies, analyzedDependencyFunc analyzedDependencyFunc) map[string]analyzedProjectDependency {
	out := make(map[string]analyzedProjectDependency)

outerDependenciesLoop:
	for pkg, depInfo := range dependencies.dependencyNames {
		// check if we've already analyzed this project before
		// ( since multiple dependencies might refer to the same repo)
		for _, prevAnalyzedDep := range out {
			if prevAnalyzedDep.githubPath == depInfo.githubPath &&
				prevAnalyzedDep.githubCommit == depInfo.githubCommit &&
				prevAnalyzedDep.workspaceVersion {
				// yes, we did.
				out[pkg] = analyzedProjectDependency{
					projectDependency:      *depInfo,
					branchName:             prevAnalyzedDep.branchName,
					fullCommitHash:         prevAnalyzedDep.fullCommitHash,
					latestBranchCommit:     prevAnalyzedDep.latestBranchCommit,
					latestBranchCommitTime: prevAnalyzedDep.latestBranchCommitTime,
					workspaceVersion:       prevAnalyzedDep.workspaceVersion,
					latestBranchVersion:    prevAnalyzedDep.latestBranchVersion,
				}
				if analyzedDependencyFunc != nil {
					analyzedDependencyFunc(pkg, out[pkg])
				}
				continue outerDependenciesLoop
			}
		}
		out[pkg] = analyzedDependency(*depInfo)

		if analyzedDependencyFunc != nil {
			analyzedDependencyFunc(pkg, out[pkg])
		}
	}

	return out
}

func analyzedDependency(depInfo projectDependency) analyzedProjectDependency {
	path := depInfo.githubPath
	if !strings.HasPrefix(path, "https://") {
		path = "https://" + path
	}
	repo, err := git.Clone(memory.NewStorage(), nil, &git.CloneOptions{
		URL:  path,
		Tags: git.AllTags,
	})
	if err != nil {
		fmt.Printf("unable to clone repository at %s\n", path)
		exitErr()
	}

	revCommit, err := lookupShortCommit(repo, depInfo.githubCommit)
	if err != nil {
		exitErr()
	}

	branches, err := getBranches(repo)
	if err != nil {
		exitErr()
	}

	latestCommitRef, err := findBranchFromCommit(repo, branches, revCommit)
	if err != nil {
		exitErr()
	}
	if latestCommitRef == nil {
		if err != nil {
			fmt.Printf("unable to find parent branch for logged commit ?! : %v\n", err)
		} else {
			fmt.Printf("unable to find parent branch for logged commit %s on %s\n", revCommit.Hash.String(), path)
		}
		exitErr()
	}
	parentBranchName := strings.ReplaceAll(latestCommitRef.Name().String(), "refs/heads/", "")

	latestCommit, err := repo.CommitObject(latestCommitRef.Hash())
	if err != nil {
		fmt.Printf("unable to get latest commit : %v\n", err)
		exitErr()
	}

	var updatedVersion string
	var workspaceVersion bool
	if depInfo.class == depClassCargo {
		// for cargo versions, we need to look into the actual repository in order to determine
		// the earliest version of the most up-to-date version.
		latestCommit, updatedVersion, workspaceVersion, err = findLatestVersion(repo, latestCommitRef, revCommit, depInfo.name)
		if err != nil {
			exitErr()
		}
	}

	return analyzedProjectDependency{
		projectDependency:      depInfo,
		branchName:             parentBranchName,
		fullCommitHash:         revCommit.Hash.String(),
		latestBranchCommit:     latestCommit.Hash.String(),
		latestBranchCommitTime: latestCommit.Committer.When.UTC(),
		latestBranchVersion:    updatedVersion,
		workspaceVersion:       workspaceVersion,
	}
}

func findBranchFromCommit(repo *git.Repository, branches map[plumbing.Hash]*plumbing.Reference, revCommit *object.Commit) (branch *plumbing.Reference, err error) {
	visited := make(map[plumbing.Hash]bool, 0)
	for len(branches) > 0 {
		for commit, branch := range branches {
			if commit.String() == revCommit.Hash.String() {
				// we found the branch.
				return branch, nil
			}
			visited[commit] = true
			delete(branches, commit)

			parentCommit, err := repo.CommitObject(commit)
			if err != nil {
				fmt.Printf("unable to get parent commit : %v\n", err)
				return nil, err
			}
			for _, parent := range parentCommit.ParentHashes {
				if !visited[parent] {
					branches[parent] = branch
				}
			}
		}
	}
	return nil, nil
}

func lookupShortCommit(repo *git.Repository, shortCommit string) (revCommit *object.Commit, err error) {
	cIter, err := repo.Log(&git.LogOptions{
		All: true,
	})
	if err != nil {
		fmt.Printf("unable to get log entries for %s: %v\n", shortCommit, err)
		return nil, err
	}

	// ... just iterates over the commits, looking for a commit with a specific hash.
	lookoutCommit := strings.ToLower(shortCommit)

	err = cIter.ForEach(func(c *object.Commit) error {
		revString := strings.ToLower(c.Hash.String())
		if strings.HasPrefix(revString, lookoutCommit) {
			// found !
			revCommit = c
			return storer.ErrStop
		}
		return nil
	})
	if err != nil && err != storer.ErrStop {
		fmt.Printf("unable to iterate on log entries : %v\n", err)
		exitErr()
	}
	if revCommit == nil {
		fmt.Printf("the commit object for short commit %s was missing ?!\n", lookoutCommit)
		exitErr()
	}
	cIter.Close()
	return revCommit, nil
}

func getBranches(repo *git.Repository) (branches map[plumbing.Hash]*plumbing.Reference, err error) {
	remoteOrigin, err := repo.Remote("origin")
	if err != nil {
		fmt.Printf("unable to retrieve origin remote : %v\n", err)
		return nil, err
	}

	remoteRefs, err := remoteOrigin.List(&git.ListOptions{})
	if err != nil {
		fmt.Printf("unable to list remote refs : %v\n", err)
		return nil, err
	}
	branchPrefix := "refs/heads/"
	branches = make(map[plumbing.Hash]*plumbing.Reference, 0)
	for _, remoteRef := range remoteRefs {
		refName := remoteRef.Name().String()
		if !strings.HasPrefix(refName, branchPrefix) {
			continue
		}
		branches[remoteRef.Hash()] = remoteRef
	}
	return branches, nil
}

func findLatestVersion(repo *git.Repository, latestCommitRef *plumbing.Reference, revCommit *object.Commit, pkgName string) (updatedLatestCommit *object.Commit, version string, workspaceVersion bool, err error) {
	// create a list of all the commits between the head and the current.
	commits := []*object.Commit{}
	headCommit, err := repo.CommitObject(latestCommitRef.Hash())
	if err != nil {
		return nil, "", false, err
	}
	for {
		commits = append(commits, headCommit)
		if headCommit.Hash == revCommit.Hash {
			// we're done.
			break
		}
		if parent, err := headCommit.Parent(0); err != nil || parent == nil {
			break
		} else {
			headCommit = parent
		}
	}

	var versions []string
	var workspaceVer []bool
	for _, commit := range commits {
		version, workspaceVersion, err := findCargoVersionForCommit(pkgName, commit)
		if err != nil {
			return nil, "", false, err
		}
		versions = append(versions, version)
		workspaceVer = append(workspaceVer, workspaceVersion)
	}
	for i := 1; i < len(versions); i++ {
		if versions[i] != versions[i-1] {
			// the version at i-1 is "newer", so we should pick that one.
			return commits[i-1], versions[i-1], workspaceVer[i-1], nil
		}
	}

	return commits[len(commits)-1], versions[len(commits)-1], workspaceVer[len(commits)-1], nil
}

//lint:ignore funlen gocyclo
func findCargoVersionForCommit(pkgName string, commit *object.Commit) (string, bool, error) {
	treeRoot, err := commit.Tree()
	if err != nil {
		return "", false, err
	}
	rootCargoFile, err := treeRoot.File("Cargo.toml")
	if err != nil {
		fmt.Printf("The package %s has unsupported repository structure\n", pkgName)
		return "", false, errors.New("unsupported repository structure")
	}
	internalWorkspacePackage := false

	rootCargoFileLines, err := rootCargoFile.Lines()
	if err != nil {
		return "", false, err
	}
	var section string
	var curPkgName string
	for _, line := range rootCargoFileLines {
		if strings.HasPrefix(line, "[") && strings.HasSuffix(line, "]") {
			section = line[1 : len(line)-1]
			continue
		}
		if strings.HasPrefix(line, "members") {
			section = "members"
			continue
		}
		switch section {
		case "members":
			if strings.Contains(line, pkgName) {
				// this is a workspace that points to an internal member;
				// the member is the package we're after.
				internalWorkspacePackage = true
			}
		case "workspace.package":
			lineParts := strings.Split(line, "=")
			if len(lineParts) != 2 {
				continue
			}
			if !strings.HasPrefix(lineParts[0], "version") {
				continue
			}
			version := strings.ReplaceAll(strings.TrimSpace(lineParts[1]), "\"", "")
			return version, true, nil
		case "package":
			lineParts := strings.Split(line, "=")
			if len(lineParts) != 2 {
				continue
			}
			if strings.HasPrefix(lineParts[0], "name") {
				curPkgName = strings.ReplaceAll(strings.TrimSpace(lineParts[1]), "\"", "")
				continue
			} else if strings.HasPrefix(lineParts[0], "version") && curPkgName == pkgName {
				version := strings.ReplaceAll(strings.TrimSpace(lineParts[1]), "\"", "")
				return version, false, nil
			}
		}
	}
	// fall-back to package specific versioning.

	if internalWorkspacePackage {
		pkgCargoFile, err := treeRoot.File(pkgName + "/Cargo.toml")
		if err != nil {
			return "", false, err
		}
		pkgCargoFileLines, err := pkgCargoFile.Lines()
		if err != nil {
			return "", false, err
		}
		var section string
		var curPkgName string
		for _, line := range pkgCargoFileLines {
			if strings.HasPrefix(line, "[") && strings.HasSuffix(line, "]") {
				section = line[1 : len(line)-1]
				continue
			}
			switch section {
			case "package":
				lineParts := strings.Split(line, "=")
				if len(lineParts) != 2 {
					continue
				}
				if strings.HasPrefix(lineParts[0], "name") {
					curPkgName = strings.ReplaceAll(strings.TrimSpace(lineParts[1]), "\"", "")
					continue
				} else if strings.HasPrefix(lineParts[0], "version") && curPkgName == pkgName {
					version := strings.ReplaceAll(strings.TrimSpace(lineParts[1]), "\"", "")
					return version, false, nil
				}
			}
		}
	}
	fmt.Printf("The package %s has unsupported repository structure\n", pkgName)
	return "", false, errors.New("unsupported repository structure")
}
