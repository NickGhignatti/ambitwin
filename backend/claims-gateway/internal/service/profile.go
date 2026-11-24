package service

import (
	"context"

	authcontracts "github.com/NickGhignatti/ambitwin/server/auth-contracts"
)

func (g *Gateway) Profile(ctx context.Context, userID string) (email, name, picture string, err error) {
	email, name, picture, err = g.keycloak.GetUser(ctx, userID)
	return email, name, picture, keycloakError(err, ErrUserNotFound)
}

func (g *Gateway) UpdateProfile(ctx context.Context, current authcontracts.StandardClaims, email, name string) (string, error) {
	if err := g.keycloak.UpdateUser(ctx, current.Sub, email, name); err != nil {
		return "", keycloakError(err, ErrEmailTaken, ErrUserNotFound)
	}
	if name != "" {
		current.AccountName = name
	}

	// Re-sign with updated claims
	return g.signer.Sign(current, g.ttl)
}

func (g *Gateway) ChangePassword(ctx context.Context, userID, currentPassword, newPassword string) error {
	email, _, _, err := g.keycloak.GetUser(ctx, userID)
	if err != nil {
		return keycloakError(err, ErrUserNotFound)
	}
	if _, err := g.keycloak.PasswordGrant(ctx, email, currentPassword); err != nil {
		return keycloakError(err, ErrInvalidCredentials)
	}
	return keycloakError(g.keycloak.ResetPassword(ctx, userID, newPassword), ErrUserNotFound)
}
