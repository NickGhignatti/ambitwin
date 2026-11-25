package keycloakadmin

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/NickGhignatti/ambitwin/server/claims-gateway/internal/service"
)

type Client struct {
	tokenURL     string
	usersURL     string
	clientID     string
	clientSecret string
	http         *http.Client
}

func New(baseURL, realm, clientID, clientSecret string) *Client {
	base := strings.TrimSuffix(baseURL, "/")
	return &Client{
		tokenURL:     base + "/realms/" + realm + "/protocol/openid-connect/token",
		usersURL:     base + "/admin/realms/" + realm + "/users",
		clientID:     clientID,
		clientSecret: clientSecret,
		http:         &http.Client{Timeout: 5 * time.Second},
	}
}

// Exchanges a username/password for an ID token via Keycloak's Resource Owner Password Credentials grant.
func (c *Client) PasswordGrant(ctx context.Context, username, password string) (string, error) {
	resp, err := c.postForm(ctx, url.Values{
		"grant_type":    {"password"},
		"client_id":     {c.clientID},
		"client_secret": {c.clientSecret},
		"username":      {username},
		"password":      {password},
		"scope":         {"openid"},
	})
	if err != nil {
		return "", fmt.Errorf("keycloak token endpoint unreachable: %w", err)
	}
	defer resp.Body.Close()

	switch resp.StatusCode {
	case http.StatusOK:
		var tok struct {
			IDToken string `json:"id_token"`
		}
		if err := json.NewDecoder(resp.Body).Decode(&tok); err != nil {
			return "", fmt.Errorf("decoding token response: %w", err)
		}
		return tok.IDToken, nil
	case http.StatusBadRequest, http.StatusUnauthorized:
		return "", service.ErrInvalidCredentials
	default:
		return "", fmt.Errorf("keycloak token endpoint returned %d", resp.StatusCode)
	}
}

func (c *Client) CreateUser(ctx context.Context, email, password, name string) error {
	userFields := map[string]any{
		"username":      email,
		"email":         email,
		"enabled":       true,
		"emailVerified": false,
		"credentials": []map[string]any{
			{"type": "password", "value": password, "temporary": false},
		},
	}
	if name != "" {
		userFields["firstName"] = name
	}
	status, err := c.adminCall(ctx, http.MethodPost, "", userFields, nil)
	if err != nil {
		return err
	}
	switch status {
	case http.StatusCreated:
		return nil
	case http.StatusConflict:
		return service.ErrEmailTaken
	default:
		return fmt.Errorf("keycloak admin API returned %d", status)
	}
}

func (c *Client) GetUser(ctx context.Context, userID string) (email, name, picture string, err error) {
	var user struct {
		Email      string `json:"email"`
		FirstName  string `json:"firstName"`
		Attributes struct {
			Picture []string `json:"picture"`
		} `json:"attributes"`
	}
	status, err := c.adminCall(ctx, http.MethodGet, "/"+userID, nil, &user)
	if err != nil {
		return "", "", "", err
	}
	switch status {
	case http.StatusOK:
		if len(user.Attributes.Picture) > 0 {
			picture = user.Attributes.Picture[0]
		}
		return user.Email, user.FirstName, picture, nil
	case http.StatusNotFound:
		return "", "", "", service.ErrUserNotFound
	default:
		return "", "", "", fmt.Errorf("keycloak admin API returned %d", status)
	}
}

func (c *Client) UpdateUser(ctx context.Context, userID, email, name string) error {
	fields := map[string]any{}
	if email != "" {
		fields["username"] = email
		fields["email"] = email
	}
	if name != "" {
		fields["firstName"] = name
	}
	status, err := c.adminCall(ctx, http.MethodPut, "/"+userID, fields, nil)
	if err != nil {
		return err
	}
	switch status {
	case http.StatusNoContent, http.StatusOK:
		return nil
	case http.StatusConflict:
		return service.ErrEmailTaken
	case http.StatusNotFound:
		return service.ErrUserNotFound
	default:
		return fmt.Errorf("keycloak admin API returned %d", status)
	}
}

func (c *Client) ResetPassword(ctx context.Context, userID, newPassword string) error {
	status, err := c.adminCall(ctx, http.MethodPut, "/"+userID+"/reset-password",
		map[string]any{"type": "password", "value": newPassword, "temporary": false}, nil)
	if err != nil {
		return err
	}
	switch status {
	case http.StatusNoContent, http.StatusOK:
		return nil
	case http.StatusNotFound:
		return service.ErrUserNotFound
	default:
		return fmt.Errorf("keycloak admin API returned %d", status)
	}
}

// adminCall sends one request to the users admin API; a 200 body is decoded into decoded when given.
func (c *Client) adminCall(ctx context.Context, method, path string, payload, decoded any) (int, error) {
	adminToken, err := c.adminToken(ctx)
	if err != nil {
		return 0, err
	}
	var body io.Reader
	if payload != nil {
		encoded, err := json.Marshal(payload)
		if err != nil {
			return 0, err
		}
		body = bytes.NewReader(encoded)
	}
	request, err := http.NewRequestWithContext(ctx, method, c.usersURL+path, body)
	if err != nil {
		return 0, err
	}
	if payload != nil {
		request.Header.Set("Content-Type", "application/json")
	}
	request.Header.Set("Authorization", "Bearer "+adminToken)
	response, err := c.http.Do(request)
	if err != nil {
		return 0, fmt.Errorf("keycloak admin API unreachable: %w", err)
	}
	defer response.Body.Close()
	if decoded != nil && response.StatusCode == http.StatusOK {
		if err := json.NewDecoder(response.Body).Decode(decoded); err != nil {
			return 0, fmt.Errorf("decoding user response: %w", err)
		}
	}
	return response.StatusCode, nil
}

func (c *Client) adminToken(ctx context.Context) (string, error) {
	resp, err := c.postForm(ctx, url.Values{
		"grant_type":    {"client_credentials"},
		"client_id":     {c.clientID},
		"client_secret": {c.clientSecret},
	})
	if err != nil {
		return "", fmt.Errorf("keycloak token endpoint unreachable: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("keycloak client-credentials grant returned %d", resp.StatusCode)
	}
	var tok struct {
		AccessToken string `json:"access_token"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&tok); err != nil {
		return "", fmt.Errorf("decoding admin token response: %w", err)
	}
	return tok.AccessToken, nil
}

func (c *Client) postForm(ctx context.Context, form url.Values) (*http.Response, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.tokenURL, strings.NewReader(form.Encode()))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	return c.http.Do(req)
}

var _ service.Keycloak = (*Client)(nil)
