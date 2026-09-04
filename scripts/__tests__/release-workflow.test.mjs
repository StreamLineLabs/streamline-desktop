import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const workflow = readFileSync(".github/workflows/release.yml", "utf8");

describe("desktop release workflow", () => {
  it("imports the Windows certificate with its password into the expected store", () => {
    expect(workflow).toContain(
      "Import-PfxCertificate -FilePath $pfxPath -CertStoreLocation " +
        "Cert:\\CurrentUser\\My -Password $password",
    );
  });

  it("pins manual builds to an explicit core revision", () => {
    expect(workflow).toContain("streamline_ref:");
    expect(workflow).toContain(
      "STREAMLINE_REF: ${{ github.ref_type == 'tag' && github.ref_name || inputs.streamline_ref }}",
    );
    expect(workflow).toContain('git clone --branch "$STREAMLINE_REF" --depth 1');
    expect(workflow).not.toMatch(
      /if \[ "\$GITHUB_REF_TYPE" != "tag" \]; then\s+REF="main"/,
    );
  });

  it("keeps manual development builds on the non-publishing path", () => {
    expect(workflow).toContain(
      "PUBLISH_RELEASE: ${{ github.event_name == 'push' && github.ref_type == 'tag' }}",
    );
    expect(workflow).toContain("if: env.PUBLISH_RELEASE != 'true'");
  });

  it("gates every build on all four desktop version authorities", () => {
    expect(workflow).toContain("verify-release-version:");
    expect(workflow).toContain("needs: verify-release-version");
    expect(workflow).toContain("node scripts/check-release-version.mjs");
    expect(workflow).toContain('RELEASE_TAG: ${{ github.event_name == \'push\' && github.ref_name || \'\' }}');
  });

  it("marks matching semantic prerelease tags as GitHub prereleases", () => {
    expect(workflow).toContain(
      "prerelease: ${{ needs.verify-release-version.outputs.prerelease == 'true' }}",
    );
    expect(workflow).not.toContain("prerelease: false");
  });
});
