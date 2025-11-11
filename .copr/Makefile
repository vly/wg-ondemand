# COPR Makefile for wg-ondemand

# This Makefile is used by COPR to build the source RPM

srpm:
	# Create source tarball
	git archive --format=tar.gz --prefix=wg-ondemand-$(VERSION)/ HEAD > wg-ondemand-$(VERSION).tar.gz

	# Build source RPM
	rpmbuild -bs \
		--define "_sourcedir $(PWD)" \
		--define "_srcrpmdir $(outdir)" \
		--define "version $(VERSION)" \
		wg-ondemand.spec
