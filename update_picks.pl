#!/usr/bin/perl

# Go through a file of
# filename<TAB>country<TAB>denomination<TAB>currency<TAB>year<TAB>Pick
# and do the following for each entry
# If there is no comment add:
#
# Country
# denomination currency year
# #pick (AI)
#
# If there is already information and no #pick, add #pick last
# else do nothing

use strict;
use Getopt::Long;

my $opt_currency_small="";
my $opt_currency_big="";
my $opt_verbose=0;
my $opt_country="";
my $opt_test;
my $file;

sub usage($)
{
  my $error=shift;
  printf STDERR "%s\n", $error;
  print "Usage: update_picks [--country=...] [--currency_small=..] [--currency_big=..] [--test] file";
  exit(1);
}

#
# Main program starts
#

my %options=('country=s' => \$opt_country,
             'currency_small=s' => \$opt_currency_small,
             'currency_big=s' => \$opt_currency_big,
             'test|t' => \$opt_test,
             'verbose+' => \$opt_verbose
            );
GetOptions(%options) or usage("Can't read options");

$file= @ARGV[0];

if (!defined($file))
{
    usage("file option is required");
}

if (!open(FILE, $file))
{
  die "Cannot open $file\n";
}

my $command= "exiftool -b -Caption-Abstract ";

while (<FILE>)
{
  my ($banknote, $banknote2, $country, $nomination, $currency, $year, $pick);
  my ($data, $new_data, $newlines);

  if (/^(.*)\t(.*)\t(.*)\t(.*)\t(.*)\t(.*)/)
  {
      $banknote= $1;
      $country= $2;
      $nomination=$3;
      $currency=$4;
      $year=$5;
      $pick=$6;
      $pick= $pick . " (AI)\n" if (!($pick eq ''));
      $banknote2=$banknote;
      $banknote2 =~ s/-A\./-B\./;

      $country= $opt_country if ($country eq '');
      $nomination= $opt_currency_big if ($nomination eq '');

      if ($opt_verbose)
      {
          print "$banknote: $country $nomination $currency $year $pick\n";
      }
      $data= `$command $banknote 2> /dev/null`;
      print "Original comment: \"$data\"\n" if ($opt_test);
      if ($data =~ /Pick#/i || $data =~ /P#/)
      {
          print "Skipped: Has pick\n" if ($opt_verbose);
          next;
      }
      $newlines = ($data =~ tr/\n//);
      if ($newlines <= 1)
      {
         $new_data= "$country\n$nomination $currency $year\n$pick\n"
      }
      elsif ($pick eq "")
      {
          print "Skipped: No pick in file not pick supplied, nothing to update\n"
              if ($opt_verbose);
          next
      }
      else
      {
          chomp($data);
          $new_data="$data\n$pick";
      }
      my ($name,$arg);
      my @names= ("caption-abstract", "imagedescription", "usercomment",
                 "notes", "comment", "description");
      $arg="";
      foreach $name (@names)
      {
        $arg= $arg . " -$name='$new_data'";
      }
      if (!$opt_test)
      {
        die if (system("/usr/bin/exiftool -v0 -overwrite_original $arg $banknote $banknote2 > /dev/null"));
      }
      else
      {
        print "exiftool arguments: \"$arg\"\n"
     }
  }
  else
  {
      print "Unparsable line: $_\n";
  }
}
