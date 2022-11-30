package test

import (
	"context"
	"errors"
	"fmt"
	"io/ioutil"
	"net/http/httptest"
	"os"
	"os/exec"
	"os/signal"
	"path"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"testing"
	"time"

	git "github.com/go-git/go-git/v5"

	"github.com/go-git/go-git/v5/plumbing/object"
	"github.com/go-git/go-git/v5/plumbing/storer"
	"github.com/go-git/go-git/v5/storage/memory"

	"github.com/stellar/go/clients/horizonclient"
	"github.com/stellar/go/clients/stellarcore"
	"github.com/stellar/go/support/log"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"

	"golang.org/x/mod/modfile"
)

const (
	StandaloneNetworkPassphrase = "Standalone Network ; February 2017"
	stellarCoreProtocolVersion  = 20
	stellarCorePort             = 11626
	goModFile                   = "go.mod"
	goMonorepoGithubPath        = "github.com/stellar/go"
)

type Test struct {
	t *testing.T

	composePath string // docker compose yml file
	composeEnv  string // docker compose env file ( used to define env var to override the go monorepo commit )

	handler       internal.Handler
	server        *httptest.Server
	horizonClient *horizonclient.Client

	coreClient *stellarcore.Client

	shutdownOnce  sync.Once
	shutdownCalls []func()
}

// The dockerComposeEnv and dockerComposeEnvMu are global objects, allowing multiple tests
// to be executed and have the dockerComposeEnv being created only once.
var dockerComposeEnv string
var dockerComposeEnvMu sync.Mutex

func NewTest(t *testing.T) *Test {
	if os.Getenv("SOROBAN_RPC_INTEGRATION_TESTS_ENABLED") == "" {
		t.Skip("skipping integration test: SOROBAN_RPC_INTEGRATION_TESTS_ENABLED not set")
	}

	composePath := findDockerComposePath()
	needBuild := false
	dockerComposeEnvMu.Lock()
	if dockerComposeEnv == "" {
		goMonorepoCommit := findGoMonorepoCommit(composePath)
		dockerComposeEnv = makeDockerComposeEnv(goMonorepoCommit)
		needBuild = true
	}
	dockerComposeEnvMu.Unlock()
	i := &Test{
		t:           t,
		composePath: composePath,
		composeEnv:  dockerComposeEnv,
	}
	if needBuild {
		i.runComposeCommand("build")
	}
	i.runComposeCommand("up", "--detach", "--quiet-pull", "--no-color")
	i.prepareShutdownHandlers()
	i.coreClient = &stellarcore.Client{URL: "http://localhost:" + strconv.Itoa(stellarCorePort)}
	i.horizonClient = &horizonclient.Client{HorizonURL: "http://localhost:8000"}
	i.waitForCore()
	i.waitForHorizon()
	i.configureJSONRPCServer()

	return i
}

func (i *Test) configureJSONRPCServer() {
	logger := log.New()

	proxy := methods.NewTransactionProxy(
		i.horizonClient,
		10,
		10,
		StandaloneNetworkPassphrase,
		2*time.Minute,
	)

	var err error
	i.handler, err = internal.NewJSONRPCHandler(internal.HandlerParams{
		AccountStore: methods.AccountStore{
			Client: i.horizonClient,
		},
		TransactionProxy: proxy,
		CoreClient:       i.coreClient,
		Logger:           logger,
	})
	if err != nil {
		i.t.Fatalf("cannot create handler: %v", err)
	}
	i.handler.Start()
	i.server = httptest.NewServer(i.handler)
}

// Runs a docker-compose command applied to the above configs
func (i *Test) runComposeCommand(args ...string) {
	integrationYaml := filepath.Join(i.composePath, "docker-compose.yml")

	cmdline := append([]string{"--env-file", i.composeEnv, "-f", integrationYaml}, args...)
	cmd := exec.Command("docker-compose", cmdline...)

	i.t.Log("Running", cmd.Env, cmd.Args)
	out, innerErr := cmd.Output()
	if exitErr, ok := innerErr.(*exec.ExitError); ok {
		fmt.Printf("stdout:\n%s\n", string(out))
		fmt.Printf("stderr:\n%s\n", string(exitErr.Stderr))
	}

	if innerErr != nil {
		i.t.Fatalf("Compose command failed: %v", innerErr)
	}
}

func (i *Test) prepareShutdownHandlers() {
	i.shutdownCalls = append(i.shutdownCalls,
		func() {
			if i.handler.Handler != nil {
				i.handler.Close()
			}
			if i.server != nil {
				i.server.Close()
			}
			i.runComposeCommand("down", "-v")
		},
	)

	// Register cleanup handlers (on panic and ctrl+c) so the containers are
	// stopped even if ingestion or testing fails.
	i.t.Cleanup(i.Shutdown)

	c := make(chan os.Signal, 1)
	signal.Notify(c, os.Interrupt, syscall.SIGTERM)
	go func() {
		<-c
		i.Shutdown()
		os.Exit(int(syscall.SIGTERM))
	}()
}

// Shutdown stops the integration tests and destroys all its associated
// resources. It will be implicitly called when the calling test (i.e. the
// `testing.Test` passed to `New()`) is finished if it hasn't been explicitly
// called before.
func (i *Test) Shutdown() {
	i.shutdownOnce.Do(func() {
		// run them in the opposite order in which they where added
		for callI := len(i.shutdownCalls) - 1; callI >= 0; callI-- {
			i.shutdownCalls[callI]()
		}
	})
}

// Wait for core to be up and manually close the first ledger
func (i *Test) waitForCore() {
	i.t.Log("Waiting for core to be up...")
	for t := 30 * time.Second; t >= 0; t -= time.Second {
		ctx, cancel := context.WithTimeout(context.Background(), time.Second)
		_, err := i.coreClient.Info(ctx)
		cancel()
		if err != nil {
			i.t.Logf("could not obtain info response: %v", err)
			time.Sleep(time.Second)
			continue
		}
		break
	}

	i.UpgradeProtocol(stellarCoreProtocolVersion)

	for t := 0; t < 5; t++ {
		ctx, cancel := context.WithTimeout(context.Background(), time.Second)
		info, err := i.coreClient.Info(ctx)
		cancel()
		if err != nil || !info.IsSynced() {
			i.t.Logf("Core is still not synced: %v %v", err, info)
			time.Sleep(time.Second)
			continue
		}
		i.t.Log("Core is up.")
		return
	}
	i.t.Fatal("Core could not sync after 30s")
}

func (i *Test) waitForHorizon() {
	for t := 60; t >= 0; t -= 1 {
		time.Sleep(time.Second)

		i.t.Log("Waiting for ingestion and protocol upgrade...")
		root, err := i.horizonClient.Root()
		if err != nil {
			i.t.Logf("could not obtain root response %v", err)
			continue
		}

		if root.HorizonSequence < 3 ||
			int(root.HorizonSequence) != int(root.IngestSequence) {
			i.t.Logf("Horizon ingesting... %v", root)
			continue
		}

		if uint32(root.CurrentProtocolVersion) == stellarCoreProtocolVersion {
			i.t.Logf("Horizon protocol version matches %d: %+v",
				root.CurrentProtocolVersion, root)
			return
		}
	}

	i.t.Fatal("Horizon not ingesting...")
}

// UpgradeProtocol arms Core with upgrade and blocks until protocol is upgraded.
func (i *Test) UpgradeProtocol(version uint32) {
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	err := i.coreClient.Upgrade(ctx, int(version))
	cancel()
	if err != nil {
		i.t.Fatalf("could not upgrade protocol: %v", err)
	}

	for t := 0; t < 10; t++ {
		ctx, cancel := context.WithTimeout(context.Background(), time.Second)
		info, err := i.coreClient.Info(ctx)
		cancel()
		if err != nil {
			i.t.Logf("could not obtain info response: %v", err)
			time.Sleep(time.Second)
			continue
		}

		if info.Info.Ledger.Version == int(version) {
			i.t.Logf("Protocol upgraded to: %d", info.Info.Ledger.Version)
			return
		}
		time.Sleep(time.Second)
	}

	i.t.Fatalf("could not upgrade protocol in 10s")
}

// Cluttering code with if err != nil is absolute nonsense.
func panicIf(err error) {
	if err != nil {
		panic(err)
	}
}

// findProjectRoot iterates upward on the directory until go.mod file is found.
func findProjectRoot(current string) string {
	// Lets you check if a particular directory contains a file.
	directoryContainsFilename := func(dir string, filename string) bool {
		files, innerErr := ioutil.ReadDir(dir)
		panicIf(innerErr)

		for _, file := range files {
			if file.Name() == filename {
				return true
			}
		}

		return false
	}
	var err error

	// In either case, we try to walk up the tree until we find "go.mod",
	// which we hope is the root directory of the project.
	for !directoryContainsFilename(current, goModFile) {
		current, err = filepath.Abs(filepath.Join(current, ".."))

		// FIXME: This only works on *nix-like systems.
		if err != nil || filepath.Base(current)[0] == filepath.Separator {
			fmt.Println("Failed to establish project root directory.")
			panic(err)
		}
	}
	return current
}

// findDockerComposePath performs a best-effort attempt to find the project's
// Docker Compose files.
func findDockerComposePath() string {

	current, err := os.Getwd()
	panicIf(err)

	//
	// We have a primary and backup attempt for finding the necessary docker
	// files: via $GOPATH and via local directory traversal.
	//

	if gopath := os.Getenv("GOPATH"); gopath != "" {
		monorepo := filepath.Join(gopath, "src", "github.com", "stellar", "soroban-tools")
		if _, err = os.Stat(monorepo); !os.IsNotExist(err) {
			current = monorepo
		}
	}

	current = findProjectRoot(current)

	// Directly jump down to the folder that should contain the configs
	return filepath.Join(current, "cmd", "soroban-rpc", "internal", "test")
}

// load the go.mod file, and extract the version of the github.com/stellar/go entry.
func loadParseGoMod(dir string) (string, error) {
	fileName := path.Join(dir, goModFile)

	cargoFileBytes, err := os.ReadFile(fileName)
	if err != nil {
		fmt.Printf("Unable to read %s file : %v\n", goModFile, err)
		return "", err
	}

	modFile, err := modfile.Parse("", cargoFileBytes, nil)
	if err != nil {
		fmt.Printf("Unable to read %s file : %v\n", goModFile, err)
		return "", err
	}
	// scan all the stellar related required modules.
	for _, require := range modFile.Require {
		if !strings.Contains(require.Mod.Path, goMonorepoGithubPath) || require.Indirect {
			continue
		}
		splittedVersion := strings.Split(require.Mod.Version, "-")
		if len(splittedVersion) != 3 {
			continue
		}
		return splittedVersion[2], nil
	}
	return "", errors.New("unable to find go monorepo")
}

func findCommitHash(shortCommitHash string) (string, error) {
	path := goMonorepoGithubPath
	if !strings.HasPrefix(path, "https://") {
		path = "https://" + path
	}
	repo, err := git.Clone(memory.NewStorage(), nil, &git.CloneOptions{
		URL: path,
	})
	if err != nil {
		fmt.Printf("unable to clone repository at %s\n", path)
		return "", err
	}

	lookoutCommit := strings.ToLower(shortCommitHash)
	cIter, err := repo.Log(&git.LogOptions{
		All:   true,
		Order: git.LogOrderCommitterTime,
	})
	if err != nil {
		fmt.Printf("unable to get log entries at %s for %s: %v\n", path, shortCommitHash, err)
		return "", err
	}
	// ... just iterates over the commits, looking for a commit with a specific hash.
	var revCommit string
	err = cIter.ForEach(func(c *object.Commit) error {
		revString := strings.ToLower(c.Hash.String())
		if strings.HasPrefix(revString, lookoutCommit) {
			// found !
			revCommit = revString
			return storer.ErrStop
		}
		return nil
	})

	if err != nil && err != storer.ErrStop {
		fmt.Printf("unable to iterate on lof entries for %s : %v\n", path, err)
		return "", err
	}
	if revCommit != "" {
		return revCommit, nil
	}
	// otherwise, this commit might be in one of the branches.
	return "", errors.New("unable to find full hash")
}

func findGoMonorepoCommit(composePath string) string {
	projectRootPath := findProjectRoot(composePath)
	shortCommitHash, err := loadParseGoMod(projectRootPath)
	panicIf(err)
	commitHash, err := findCommitHash(shortCommitHash)
	panicIf(err)

	return commitHash
}

func makeDockerComposeEnv(goMonorepoCommit string) string {
	file, err := os.CreateTemp(os.TempDir(), "docker-compose-go-monorepo")
	if err != nil {
		fmt.Printf("Unable to create temporary file : %v\n", err)
		panic(err)
	}
	fmt.Fprintf(file, "GOMONOREPO_COMMIT=%s\n", goMonorepoCommit)
	err = file.Close()
	if err != nil {
		fmt.Printf("Unable to close temporary file : %v\n", err)
		panic(err)
	}
	return file.Name()
}
