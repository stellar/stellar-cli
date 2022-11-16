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

func printDependencies(dependencies map[string]projectDependency) {
	for pkg, dep := range dependencies {
		fmt.Printf("%s %s %s[%s%s%s@%s%s%s]%s\n",
			colorGreen,
			pkg,
			colorYellow,
			colorCyan,
			dep.githubPath,
			colorWhite,
			colorPurple,
			dep.githubCommit,
			colorYellow,
			colorReset)
	}
}

func analyzedDepPrinter(pkg string, dep analyzedProjectDependency) {
	if dep.fullCommitHash == dep.latestBranchCommit {
		fmt.Printf("%s %s %s[%s%s%s@%s%s%s]%s\n",
			colorGreen,
			pkg,
			colorYellow,
			colorCyan,
			dep.githubPath,
			colorWhite,
			colorPurple,
			dep.githubCommit,
			colorYellow,
			colorReset)
		return
	}
	fmt.Printf("%s %s %s[%s%s%s@%s%s%s]%s Upgrade %s[%s%s%s]%s\n",
		colorGreen,
		pkg,
		colorYellow,
		colorCyan,
		dep.githubPath,
		colorWhite,
		colorPurple,
		dep.githubCommit,
		colorYellow,
		colorReset,
		colorYellow,
		colorPurple,
		dep.latestBranchCommit[:len(dep.githubCommit)],
		colorYellow,
		colorReset,
	)
}
