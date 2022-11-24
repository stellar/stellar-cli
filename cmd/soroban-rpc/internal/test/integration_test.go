package test

import (
	"fmt"
	"testing"
)

func TestFindDockerComposePath(t *testing.T) {
	dockerPath := findDockerComposePath()

	if len(dockerPath) == 0 {
		t.Fail()
	}
	fmt.Printf("docker compose path is %s\n", dockerPath)
}

func TestFindGoMonorepoCommit(t *testing.T) {
	dockerPath := findDockerComposePath()

	if len(dockerPath) == 0 {
		t.Fail()
	}
	monoRepoCommit := findGoMonorepoCommit(dockerPath)
	if monoRepoCommit == "" {
		fmt.Printf("failed to find go monorepo commit\n")
		t.Fail()
	}
}
