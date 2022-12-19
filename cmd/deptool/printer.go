package main

import "fmt"

const (
	colorReset = "\033[0m"
	//colorRed = "\033[31m"
	colorGreen  = "\033[32m"
	colorYellow = "\033[33m"
	//colorBlue = "\033[34m"
	colorPurple = "\033[35m"
	colorCyan   = "\033[36m"
	colorWhite  = "\033[37m"
)

func printDependencies(dependencies *projectDependencies) {
	for _, dep := range dependencies.dependencies {
		var version string
		if dep.version != "" {
			version = fmt.Sprintf(" %s%s", colorGreen, dep.version)
		}
		fmt.Printf("%s %s %s[%s%s%s@%s%s%s%s]%s\n",
			colorGreen,
			dep.name,
			colorYellow,
			colorCyan,
			dep.githubPath,
			colorWhite,
			colorPurple,
			dep.githubCommit,
			version,
			colorYellow,
			colorReset)
	}
}

func analyzedDepPrinter(pkg string, dep analyzedProjectDependency) {
	var version, latestBranchVersion string
	if dep.version != "" {
		version = fmt.Sprintf(" %s%s", colorGreen, dep.version)
	}
	// do we have an upgrade ?
	if dep.fullCommitHash == dep.latestBranchCommit {
		fmt.Printf("%s %s %s[%s%s%s@%s%s%s%s]%s\n",
			colorGreen,
			pkg,
			colorYellow,
			colorCyan,
			dep.githubPath,
			colorWhite,
			colorPurple,
			dep.githubCommit,
			version,
			colorYellow,
			colorReset)
		return
	}

	if dep.latestBranchVersion != "" {
		latestBranchVersion = fmt.Sprintf(" %s%s", colorGreen, dep.latestBranchVersion)
	}
	fmt.Printf("%s %s %s[%s%s%s@%s%s%s%s]%s Upgrade %s[%s%s%s%s]%s\n",
		colorGreen,
		pkg,
		colorYellow,
		colorCyan,
		dep.githubPath,
		colorWhite,
		colorPurple,
		dep.githubCommit,
		version,
		colorYellow,
		colorReset,
		colorYellow,
		colorPurple,
		dep.latestBranchCommit[:len(dep.githubCommit)],
		latestBranchVersion,
		colorYellow,
		colorReset,
	)
}
