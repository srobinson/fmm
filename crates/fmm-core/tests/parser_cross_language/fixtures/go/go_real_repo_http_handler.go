
package handlers

import (
    "encoding/json"
    "net/http"
    "log"
)

type Response struct {
    Status  string      `json:"status"`
    Data    interface{} `json:"data,omitempty"`
    Error   string      `json:"error,omitempty"`
}

type Handler struct {
    logger *log.Logger
}

func NewHandler(logger *log.Logger) *Handler {
    return &Handler{logger: logger}
}

func (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
    h.logger.Printf("Request: %s %s", r.Method, r.URL.Path)
    json.NewEncoder(w).Encode(Response{Status: "ok"})
}

func healthCheck(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusOK)
}
