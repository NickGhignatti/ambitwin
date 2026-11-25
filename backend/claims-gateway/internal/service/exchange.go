package service

import (
	"context"
	"errors"
	"fmt"
	"time"

	authcontracts "github.com/NickGhignatti/ambitwin/server/auth-contracts"
)

var (
	ErrInvalidIDToken     = errors.New("invalid id token")
	ErrTenancyUnavailable = errors.New("tenancy unavailable")
	ErrInviteOnly         = errors.New("domain is invite-only")
)

// IDTokenClaims is what the gateway needs out of a verified IdP token.
type IDTokenClaims struct {
	Sub               string
	PreferredUsername string
	Name              string // Keycloak's "full name" claim; "" if the account has no first/last name set
	EmailVerified     bool
	Organization      string // Keycloak Organizations claim; "" if the user belongs to none yet
	Roles             []string
}

type Verifier interface {
	Verify(ctx context.Context, rawIDToken string) (IDTokenClaims, error)
}

type ProvisionRequest struct {
	AccountID  string
	DomainName string
	Role       string
	ExternalID string
}

type TenancyClient interface {
	MembershipsFor(ctx context.Context, accountID string) ([]authcontracts.Membership, error)
	Provision(ctx context.Context, req ProvisionRequest) error
}

type Signer interface {
	Sign(claims authcontracts.StandardClaims, ttl time.Duration) (string, error)
}

// Keycloak is the sign-in server's account API: password sign-in and self-service account changes.
type Keycloak interface {
	PasswordGrant(ctx context.Context, username, password string) (idToken string, err error)
	CreateUser(ctx context.Context, email, password, name string) error
	GetUser(ctx context.Context, userID string) (email, name, picture string, err error)
	UpdateUser(ctx context.Context, userID, email, name string) error
	// ResetPassword sets the password without checking the old one; the caller checks it first.
	ResetPassword(ctx context.Context, userID, newPassword string) error
}

type Gateway struct {
	verifier Verifier
	tenancy  TenancyClient
	signer   Signer
	keycloak Keycloak
	ttl      time.Duration
}

func New(verifier Verifier, tenancy TenancyClient, signer Signer, keycloak Keycloak, ttl time.Duration) *Gateway {
	return &Gateway{verifier: verifier, tenancy: tenancy, signer: signer, keycloak: keycloak, ttl: ttl}
}

// Exchange turns a verified ID token into a gateway token, creating the membership on first login.
func (g *Gateway) Exchange(ctx context.Context, rawIDToken string) (string, error) {
	identity, err := g.verifier.Verify(ctx, rawIDToken)
	if err != nil {
		return "", fmt.Errorf("%w: %v", ErrInvalidIDToken, err)
	}

	memberships, err := g.tenancy.MembershipsFor(ctx, identity.Sub)
	if err != nil {
		return "", fmt.Errorf("%w: %v", ErrTenancyUnavailable, err)
	}

	// first login from an organization: tenancy has no record yet
	if len(memberships) == 0 && identity.Organization != "" {
		memberships, err = g.provisionOnFirstLogin(ctx, identity)
		if err != nil {
			return "", err
		}
	}

	return g.signer.Sign(authcontracts.StandardClaims{
		Sub:         identity.Sub,
		AccountName: accountName(identity),
		Memberships: memberships,
	}, g.ttl)
}

func (g *Gateway) RefreshSession(ctx context.Context, current authcontracts.StandardClaims) (string, error) {
	memberships, err := g.tenancy.MembershipsFor(ctx, current.Sub)
	if err != nil {
		return "", fmt.Errorf("%w: %v", ErrTenancyUnavailable, err)
	}
	current.Memberships = memberships
	return g.signer.Sign(current, g.ttl)
}

func accountName(identity IDTokenClaims) string {
	if identity.Name != "" {
		return identity.Name
	}
	return identity.PreferredUsername
}

func (g *Gateway) provisionOnFirstLogin(ctx context.Context, identity IDTokenClaims) ([]authcontracts.Membership, error) {
	role := HighestRole(identity.Roles)
	err := g.tenancy.Provision(ctx, ProvisionRequest{
		AccountID: identity.Sub, DomainName: identity.Organization, Role: role, ExternalID: identity.Sub,
	})
	switch {
	case err == nil:
		return []authcontracts.Membership{{Domain: identity.Organization, Role: role}}, nil
	case errors.Is(err, ErrInviteOnly):
		// invite-only domain: sign in with no membership instead of failing
		return []authcontracts.Membership{}, nil
	default:
		return nil, fmt.Errorf("%w: %v", ErrTenancyUnavailable, err)
	}
}

func HighestRole(roles []string) string {
	best, bestWeight := "standard_customer", authcontracts.RoleWeights["standard_customer"]
	for _, role := range roles {
		if weight, known := authcontracts.RoleWeights[role]; known && weight > bestWeight {
			best, bestWeight = role, weight
		}
	}
	return best
}
