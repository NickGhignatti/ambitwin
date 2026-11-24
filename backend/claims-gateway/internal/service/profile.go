package service

import (
	"context"

	authcontracts "github.com/NickGhignatti/ambitwin/server/auth-contracts"
)

// ProfileReader reads a Keycloak user's current email/display name/picture.
type ProfileReader interface {
	GetUser(ctx context.Context, userID string) (email, name, picture string, err error)
}

// ProfileUpdater patches a Keycloak user's email and/or display name.
type ProfileUpdater interface {
	UpdateUser(ctx context.Context, userID, email, name string) error
}

// PasswordChanger sets a Keycloak user's password directly — the caller is responsible
// for verifying the current password first, since this has no way to do that itself.
type PasswordChanger interface {
	ResetPassword(ctx context.Context, userID, newPassword string) error
}

// WithProfileManagement wires the optional self-service account-settings
// dependencies onto an already-constructed Gateway.
func (g *Gateway) WithProfileManagement(r ProfileReader, u ProfileUpdater, p PasswordChanger) *Gateway {
	g.profileReader = r
	g.profileUpdater = u
	g.passwordChanger = p
	return g
}

func (g *Gateway) Profile(ctx context.Context, userID string) (email, name, picture string, err error) {
	email, name, picture, err = g.profileReader.GetUser(ctx, userID)
	return email, name, picture, keycloakError(err, ErrUserNotFound)
}

func (g *Gateway) UpdateProfile(ctx context.Context, current authcontracts.StandardClaims, email, name string) (string, error) {
	if err := g.profileUpdater.UpdateUser(ctx, current.Sub, email, name); err != nil {
		return "", keycloakError(err, ErrEmailTaken, ErrUserNotFound)
	}
	if name != "" {
		current.AccountName = name
	}

	// Re-sign with updated claims
	return g.signer.Sign(current, g.ttl)
}

func (g *Gateway) ChangePassword(ctx context.Context, userID, currentPassword, newPassword string) error {
	email, _, _, err := g.profileReader.GetUser(ctx, userID)
	if err != nil {
		return keycloakError(err, ErrUserNotFound)
	}
	if _, err := g.authenticator.PasswordGrant(ctx, email, currentPassword); err != nil {
		return keycloakError(err, ErrInvalidCredentials)
	}
	return keycloakError(g.passwordChanger.ResetPassword(ctx, userID, newPassword), ErrUserNotFound)
}
