Name:           engram
Version:        0.1.0
Release:        1%{?dist}
Summary:        ENGRAM — Engineering Intelligence, etched in Notion

License:        MIT
URL:            https://github.com/manojpisini/engram

%description
AI-powered engineering intelligence that connects GitHub, Notion,
and Claude to provide automated code review, security audits,
performance tracking, and team health monitoring.

%install
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/etc/engram
mkdir -p %{buildroot}/usr/lib/systemd/system
mkdir -p %{buildroot}/usr/share/engram/dashboard

cp %{_sourcedir}/engram %{buildroot}/usr/bin/engram
cp %{_sourcedir}/engram.toml %{buildroot}/etc/engram/engram.toml
cp %{_sourcedir}/engram.service %{buildroot}/usr/lib/systemd/system/engram.service
cp -r %{_sourcedir}/dashboard/* %{buildroot}/usr/share/engram/dashboard/

%files
%attr(755, root, root) /usr/bin/engram
%config(noreplace) /etc/engram/engram.toml
/usr/lib/systemd/system/engram.service
/usr/share/engram/dashboard

%pre
getent passwd engram >/dev/null || useradd --system --no-create-home --shell /sbin/nologin engram

%post
mkdir -p /var/log/engram
chown engram:engram /var/log/engram
chown -R engram:engram /etc/engram
systemctl daemon-reload
systemctl enable engram.service
systemctl start engram.service || true
echo ""
echo "ENGRAM installed! Open http://localhost:3000 to configure."
echo ""

%preun
systemctl stop engram.service || true
systemctl disable engram.service || true
