package config

import (
	"cmp"
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/pem"
	"fmt"
	"log"
	"os"
	"strings"
	"time"
)

type Config struct {
	Addr                 string
	OIDCDiscoveryURL     string // how THIS process reaches the IdP (e.g. internal Docker hostname)
	OIDCIssuer           string // the canonical `iss` browsers' tokens carry (may differ from the above)
	OIDCClientID         string
	TenancyURL           string
	InternalSecret       []byte
	Issuer               string // this gateway's own iss claim, e.g. "cv-gateway"
	TokenTTL             time.Duration
	SigningKey           *rsa.PrivateKey
	SigningKeyID         string
	KeycloakBaseURL      string // OIDCDiscoveryURL with the "/realms/<realm>" suffix stripped
	KeycloakRealm        string
	KeycloakClientID     string
	KeycloakClientSecret string
}

func Load() (Config, error) {
	var missing []string
	required := func(name string) string {
		value := os.Getenv(name)
		if value == "" {
			missing = append(missing, name)
		}
		return value
	}
	issuer := required("OIDC_ISSUER")
	clientID := required("OIDC_CLIENT_ID")
	tenancyURL := required("TENANCY_URL")
	secret := required("INTERNAL_SIGNING_SECRET")
	keycloakRealm := required("KEYCLOAK_REALM")
	keycloakClientID := required("REGISTRATION_CLIENT_ID")
	keycloakClientSecret := required("REGISTRATION_CLIENT_SECRET")
	if len(missing) > 0 {
		return Config{}, fmt.Errorf("missing required env: %s", strings.Join(missing, ", "))
	}
	// single-hostname deployments use one URL for both
	discoveryURL := cmp.Or(os.Getenv("OIDC_DISCOVERY_URL"), issuer)

	key, keyID, err := loadOrGenerateKey()
	if err != nil {
		return Config{}, err
	}

	addr := cmp.Or(os.Getenv("ADDR"), ":3000")
	gatewayIssuer := cmp.Or(os.Getenv("GATEWAY_ISSUER"), "cv-gateway")

	keycloakBaseURL := strings.TrimSuffix(discoveryURL, "/realms/"+keycloakRealm)

	return Config{
		Addr: addr, OIDCDiscoveryURL: discoveryURL, OIDCIssuer: issuer, OIDCClientID: clientID,
		TenancyURL: tenancyURL, InternalSecret: []byte(secret),
		Issuer: gatewayIssuer, TokenTTL: 15 * time.Minute,
		SigningKey: key, SigningKeyID: keyID,
		KeycloakBaseURL: keycloakBaseURL, KeycloakRealm: keycloakRealm,
		KeycloakClientID: keycloakClientID, KeycloakClientSecret: keycloakClientSecret,
	}, nil
}

// loadOrGenerateKey reads the PKCS#8 RSA key at GATEWAY_PRIVATE_KEY_FILE,
// or makes a throwaway one when unset: every restart then signs everyone out.
func loadOrGenerateKey() (*rsa.PrivateKey, string, error) {
	if keyPath := os.Getenv("GATEWAY_PRIVATE_KEY_FILE"); keyPath != "" {
		pemBytes, err := os.ReadFile(keyPath)
		if err != nil {
			return nil, "", fmt.Errorf("reading GATEWAY_PRIVATE_KEY_FILE: %w", err)
		}
		block, _ := pem.Decode(pemBytes)
		if block == nil {
			return nil, "", fmt.Errorf("GATEWAY_PRIVATE_KEY_FILE is not valid PEM")
		}
		key, err := x509.ParsePKCS8PrivateKey(block.Bytes)
		if err != nil {
			return nil, "", fmt.Errorf("parsing GATEWAY_PRIVATE_KEY_FILE: %w", err)
		}
		rsaKey, ok := key.(*rsa.PrivateKey)
		if !ok {
			return nil, "", fmt.Errorf("GATEWAY_PRIVATE_KEY_FILE is not an RSA key")
		}
		return rsaKey, cmp.Or(os.Getenv("GATEWAY_KEY_ID"), "cv-gateway-1"), nil
	}

	log.Println("WARNING: GATEWAY_PRIVATE_KEY_FILE not set — generating an ephemeral key. " +
		"Every restart invalidates existing sessions and JWKS caches. Fine for local dev, never for a real cell.")
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		return nil, "", err
	}
	return key, "ephemeral-dev-key", nil
}
