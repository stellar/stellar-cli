package main

import (
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
}

type analyzedDependencyFunc func(string, analyzedProjectDependency)

func analyze(dependencies map[string]projectDependency, analyzedDependencyFunc analyzedDependencyFunc) map[string]analyzedProjectDependency {
	out := make(map[string]analyzedProjectDependency)

outerDependenciesLoop:
	for pkg, depInfo := range dependencies {
		// check if we've already analyzed this project before
		// ( since multiple dependencies might refer to the same repo)
		for _, prevAnalyzedDep := range out {
			if prevAnalyzedDep.githubPath == depInfo.githubPath && prevAnalyzedDep.githubCommit == depInfo.githubCommit {
				// yes, we did.
				out[pkg] = analyzedProjectDependency{
					projectDependency:      depInfo,
					branchName:             prevAnalyzedDep.branchName,
					fullCommitHash:         prevAnalyzedDep.fullCommitHash,
					latestBranchCommit:     prevAnalyzedDep.latestBranchCommit,
					latestBranchCommitTime: prevAnalyzedDep.latestBranchCommitTime,
				}
				if analyzedDependencyFunc != nil {
					analyzedDependencyFunc(pkg, out[pkg])
				}
				continue outerDependenciesLoop
			}
		}
		out[pkg] = analyzedDependency(depInfo)

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
		URL: path,
	})
	if err != nil {
		fmt.Printf("unable to clone repository at %s\n", path)
		exitErr()
	}

	cIter, err := repo.Log(&git.LogOptions{})
	if err != nil {
		fmt.Printf("unable to get log entries at %s for %s: %v\n", path, depInfo.githubCommit, err)
		exitErr()
	}

	// ... just iterates over the commits, looking for a commit with a specific hash.
	lookoutCommit := strings.ToLower(depInfo.githubCommit)
	var revCommit *object.Commit
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
		fmt.Printf("unable to iterate on lof entries for %s : %v\n", path, err)
		exitErr()
	}

	branchesIt, err := repo.Branches()
	if err != nil {
		fmt.Printf("unable to iterate on rpo entries for %s : %v\n", path, err)
		exitErr()
	}

	branches := make(map[plumbing.Hash]*plumbing.Reference, 0)
	branchTime := make(map[*plumbing.Reference]time.Time, 0)
	err = branchesIt.ForEach(func(c *plumbing.Reference) error {
		parentCommit, err := repo.CommitObject(c.Hash())
		if err != nil {
			fmt.Printf("unable to iterate on repo entries for %s : %v\n", path, err)
			exitErr()
		}
		branches[parentCommit.Hash] = c
		branchTime[c] = parentCommit.Committer.When
		return nil
	})

	var parentBranch *plumbing.Reference
	for len(branches) > 0 {
		for commit, branch := range branches {
			if commit.String() == revCommit.Hash.String() {
				// we found the branch.
				parentBranch = branch
				branches = nil
				break
			}
			delete(branches, commit)

			parentCommit, err := repo.CommitObject(commit)
			if err != nil {
				fmt.Printf("unable to get parent commit : %v\n", err)
				exitErr()
			}
			for _, parent := range parentCommit.ParentHashes {
				branches[parent] = branch
			}
		}
	}
	if parentBranch == nil {
		fmt.Printf("unable to find parent branch for logged commit ?! : %v\n", err)
		exitErr()
	}
	parentBranchName := parentBranch.String()

	return analyzedProjectDependency{
		projectDependency:      depInfo,
		branchName:             parentBranchName,
		fullCommitHash:         revCommit.Hash.String(),
		latestBranchCommit:     parentBranch.Hash().String(),
		latestBranchCommitTime: branchTime[parentBranch].UTC(),
	}
}
