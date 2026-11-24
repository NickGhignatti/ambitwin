package main

import (
	"context"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/MicahParks/keyfunc/v3"

	"github.com/NickGhignatti/ambitwin/server/claims-gateway/internal/api"
	"github.com/NickGhignatti/ambitwin/server/claims-gateway/internal/config"
	"github.com/NickGhignatti/ambitwin/server/claims-gateway/internal/keycloakadmin"
	"github.com/NickGhignatti/ambitwin/server/claims-gateway/internal/oidcverifier"
	"github.com/NickGhignatti/ambitwin/server/claims-gateway/internal/service"
	"github.com/NickGhignatti/ambitwin/server/claims-gateway/internal/signer"
	"github.com/NickGhignatti/ambitwin/server/claims-gateway/internal/tenancyclient"
)

func main() {
	settings, err := config.Load()
	if err != nil {
		log.Fatalf("config: %v", err)
	}

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	// OIDC discovery against Keycloak, fail-fast if unreachable at boot.
	verifier, err := oidcverifier.New(ctx, settings.OIDCDiscoveryURL, settings.OIDCIssuer, settings.OIDCClientID)
	if err != nil {
		log.Fatalf("oidc discovery: %v", err)
	}
	tenancy := tenancyclient.New(settings.TenancyURL, settings.InternalSecret)
	tokenSigner := signer.New(settings.SigningKey, settings.SigningKeyID, settings.Issuer)

	// keycloakadmin holds the confidential cv-gateway client secret and is
	// the only thing that ever talks to Keycloak's token/admin endpoints.
	keycloakClient := keycloakadmin.New(settings.KeycloakBaseURL, settings.KeycloakRealm, settings.KeycloakClientID, settings.KeycloakClientSecret)
	gateway := service.New(verifier, tenancy, tokenSigner, keycloakClient, settings.TokenTTL)

	// /me verifies the gateway's OWN minted tokens.
	publicKeySet := tokenSigner.JWKS()
	verifyKeys, err := keyfunc.NewJWKSetJSON(publicKeySet)
	if err != nil {
		log.Fatalf("building self-verification keyfunc: %v", err)
	}
	router := api.Mount(gateway, publicKeySet, verifyKeys, settings.Issuer)

	server := &http.Server{Addr: settings.Addr, Handler: router}
	go func() {
		log.Printf("claims-gateway listening on %s (issuer=%s)", settings.Addr, settings.OIDCIssuer)
		if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatalf("server: %v", err)
		}
	}()

	<-ctx.Done()
	shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	_ = server.Shutdown(shutdownCtx)
}
