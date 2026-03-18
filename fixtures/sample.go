package server

import (
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/gin-gonic/gin"
	"github.com/redis/go-redis/v9"
)

// MaxRetries is the maximum number of retries for failed operations.
const MaxRetries = 3

const internalTimeout = 30

// Status represents the state of a handler.
type Status int

const (
	StatusActive Status = iota
	StatusInactive
)

// Config holds the server configuration.
type Config struct {
	Host    string
	Port    int
	Debug   bool
}

type privateState struct {
	counter int
}

// Handler processes incoming requests.
type Handler struct {
	config Config
	state  privateState
	client *redis.Client
}

// NewHandler creates a new Handler with the given config.
func NewHandler(cfg Config) *Handler {
	return &Handler{
		config: cfg,
		client: redis.NewClient(&redis.Options{
			Addr: fmt.Sprintf("%s:%d", cfg.Host, cfg.Port),
		}),
	}
}

func (h *Handler) validate() error {
	if h.config.Host == "" {
		return fmt.Errorf("host is required")
	}
	return nil
}

// Process handles an incoming HTTP request.
func Process(w http.ResponseWriter, r *http.Request) {
	data := map[string]string{"status": "ok"}
	json.NewEncoder(w).Encode(data)
}

func helperFunc() string {
	return "internal"
}
